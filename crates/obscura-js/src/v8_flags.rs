use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Once;

static INIT: Once = Once::new();
/// Set once the first `ObscuraJsRuntime` (and thus the V8 platform) has been
/// constructed. A `set_v8_flags` call after this point is dropped rather than
/// aborting the process.
static PLATFORM_STARTED: AtomicBool = AtomicBool::new(false);

/// Record that a JS runtime is being constructed, so any later `set_v8_flags`
/// call is refused instead of reaching V8's fatal late-flag path. Called at the
/// top of runtime construction, before the platform is initialized.
pub(crate) fn mark_platform_started() {
    PLATFORM_STARTED.store(true, Ordering::SeqCst);
}

/// Set ICU's process timezone before any isolate exists. Windows ICU ignores
/// TZ, so setting the environment alone cannot keep Date and Intl coherent.
/// Call after configuring V8 flags and startup environment variables.
#[cfg(windows)]
pub fn set_process_timezone(zone: &str) -> Result<(), String> {
    if zone.is_empty() || zone.contains('\0') {
        return Err("timezone must be a nonempty IANA identifier without NUL bytes".into());
    }
    let mut input: Vec<u16> = zone.encode_utf16().collect();
    let length = i32::try_from(input.len()).map_err(|_| "timezone identifier is too long")?;
    input.push(0);
    if PLATFORM_STARTED.swap(true, Ordering::SeqCst) {
        return Err("timezone must be configured before the V8 platform starts".into());
    }
    // deno_core initializes the bundled ICU data before starting V8. No Date
    // cache has been created yet, so both Date and Intl will use this default.
    deno_core::JsRuntime::init_platform(None, false);
    let mut canonical = [0u16; 512];
    let mut is_system_id: i8 = 0;
    let mut error: i32 = 0;
    // SAFETY: buffers remain alive, lengths match their allocations, and the
    // bundled rusty_v8 ICU is version 74 (the same ABI used by v8::icu).
    let canonical_length = unsafe {
        ucal_getCanonicalTimeZoneID_74(
            input.as_ptr(), length, canonical.as_mut_ptr(), canonical.len() as i32,
            &mut is_system_id, &mut error,
        )
    };
    if error > 0 || is_system_id == 0 || canonical_length <= 0
        || canonical_length as usize >= canonical.len()
    {
        return Err(format!("invalid IANA timezone {zone:?} (ICU error {error})"));
    }
    error = 0;
    // SAFETY: ICU returned a NUL-terminated identifier into the bounded buffer.
    unsafe { ucal_setDefaultTimeZone_74(canonical.as_ptr(), &mut error) };
    if error > 0 {
        return Err(format!("cannot set timezone {zone:?} (ICU error {error})"));
    }
    Ok(())
}

#[cfg(windows)]
unsafe extern "C" {
    fn ucal_getCanonicalTimeZoneID_74(
        id: *const u16, length: i32, result: *mut u16, capacity: i32,
        is_system_id: *mut i8, error: *mut i32,
    ) -> i32;
    fn ucal_setDefaultTimeZone_74(id: *const u16, error: *mut i32);
}

/// Apply user-supplied V8 flags exactly once, before the first isolate is
/// created.
///
/// `flags` is a raw V8 flag string in the same form V8/Chromium/Node accept
/// (e.g. `"--max-old-space-size=4096 --max-semi-space-size=64"`). An empty or
/// whitespace-only string is a no-op and does not consume the one-shot guard,
/// so a later non-empty call still takes effect.
///
/// V8 ignores `set_flags_from_string` once the platform is initialized, so the
/// first non-empty call must run before any `JsRuntime` is constructed.
/// Subsequent calls are silently dropped.
pub fn set_v8_flags(flags: &str) {
    let trimmed = flags.trim();
    if trimmed.is_empty() {
        return;
    }
    if PLATFORM_STARTED.load(Ordering::SeqCst) {
        // Once an isolate exists, V8's SetFlagsFromCommandLine calls V8_Fatal
        // and aborts the whole process (SIGTRAP) — it does NOT silently ignore
        // the call as the platform docs imply. Drop the late call with a
        // warning instead of crashing the embedder; flags only take effect
        // before the first ObscuraJsRuntime is constructed.
        tracing::warn!(
            "set_v8_flags({trimmed:?}) ignored: a JS runtime already exists — \
             V8 flags must be set before the first ObscuraJsRuntime is constructed"
        );
        return;
    }
    INIT.call_once(|| {
        deno_core::v8::V8::set_flags_from_string(trimmed);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_is_noop() {
        // Must not panic and must not consume the Once guard.
        set_v8_flags("");
        set_v8_flags("   ");
        set_v8_flags("\t\n");
    }

    // #853 — a set_v8_flags call after a runtime exists used to abort the whole
    // process (V8_Fatal / SIGTRAP). Constructing a real runtime initializes the
    // V8 platform (and marks it started); the subsequent set_v8_flags must be
    // dropped with a warning rather than crashing. Reaching the assertion at all
    // (no SIGTRAP) is the regression check.
    #[test]
    fn late_call_after_a_runtime_exists_does_not_abort() {
        let _rt = crate::runtime::ObscuraJsRuntime::new();
        set_v8_flags("--max-old-space-size=32");
        assert!(PLATFORM_STARTED.load(Ordering::SeqCst));
    }

    #[cfg(windows)]
    #[test]
    fn timezone_change_after_runtime_creation_is_rejected() {
        let _rt = crate::runtime::ObscuraJsRuntime::new();
        assert!(set_process_timezone("Europe/Berlin").unwrap_err().contains("before"));
    }
}
