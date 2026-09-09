# Use a coherent Chrome 149 browser identity

Status: accepted

Obscura is not Chromium, so its browser identity must be kept coherent across the `wreq`/BoringSSL transport, HTTP client hints, JavaScript navigator surfaces, and CDP metadata. We use the newest profile supported by the pinned `wreq-util` release, Chrome 149 via `wreq-util` rc.14, and update every fallback identity with it; we do not claim unsupported WebGL/WebRTC capabilities or add detector-only values. Raw-direct BrowserScan and CreepJS runs are diagnostic evidence only, while the offline obstacle course remains the behavioral regression gate.

The stealth profile also uses the observed normal Chrome color preference (dark): `matchMedia`, CSS media-query selection, and `light-dark()` share one setting. Non-stealth runtimes retain the historical light default so this identity correction does not alter the engine's existing baseline lane.

## Windows-only identity during profile unification

Date: 2026-09-09

Until selectable identities are wired through HTTP, TLS, JavaScript, child
realms, and CDP from one source of truth, the supported profile table contains
only Chrome 149 on Windows. The legacy `OBSCURA_PROFILE` and
`OBSCURA_ROTATE_PROFILE` settings are ignored. This keeps the host OS separate
from the reported identity and avoids advertising a macOS profile that only
some runtime surfaces can represent.

Explicit custom User-Agent APIs remain available for compatibility tests, but
they are low-level overrides outside the supported identity contract. They are
not a mechanism for selecting a complete browser profile.

`Function.prototype.toString` is installed per V8 realm as a native `FunctionTemplate` with its prototype removed. Its formatter still delegates to the realm's source map for JS-implemented browser APIs. The exposed `name` remains `toString`, while the V8 class identity remains `Function`, matching Chrome's intrinsic `instanceof` error frame without changing CreepJS itself or hard-coding a detector result.

## Timezone and detector investigation

Date: 2026-09-09

### Hypothesis

The conflicting `Europe/Berlin` and `Africa/Algiers` receipts could come from profile initialization, ICU, stale realm state, or separate execution paths. The first check is whether the CLI's process timezone is the common source for `Intl`, `Date`, iframes, and workers.

### Evidence before

- Binary: `/tmp/obscura-consistency-20260909/obscura-before-6714a6e1`, SHA-256 `6714a6e15dd962dfd10725da09229448cea92c53a38634a570a2f1270311444b`.
- Host zone: `/var/db/timezone/zoneinfo/Africa/Algiers`; no `TZ` environment variable was set.
- With no timezone override, the CLI's existing fallback set every realm to `Europe/Berlin`. Main page, iframe, and worker all reported `Intl.DateTimeFormat().resolvedOptions().timeZone = Europe/Berlin`; a July probe reported offset `-120`.
- With `TZ=Africa/Algiers` or `OBSCURA_TIMEZONE=Africa/Algiers`, the same three realms all reported `Africa/Algiers` and offset `-60`. This reproduces the older Algiers receipt without changing source or profile state.
- The direct probe also matched UA/platform, en-US locale, dark mode, five plugins, two MIME types, webdriver false, and native-shaped `Function.prototype.toString` in main, iframe, and worker realms.
- CreepJS with the default fallback completed with `6% like headless`, `0% headless`, `0% stealth`, `hasToStringProxy: false`, and `Europe/Berlin (-120)`. CreepJS with the explicit Algiers override completed with `0% like headless`, `0% headless`, `0% stealth`, `hasToStringProxy: false`, and `Africa/Algiers (-60)`. Both runs logged the existing `getBestRect(...).height` compatibility error.
- BrowserScan's first-party ES-module graph loaded. Stealth mode logged two HTTP-0 dynamic script errors for third-party tracker/advertising scripts blocked by the tracker list. Non-stealth removed those errors but still left result widgets blank and produced a separate intermittent page/bundle error. No BrowserScan pass is claimed.

### Source-level change

None. The contradiction is caused by launch configuration: the CLI deliberately uses `Europe/Berlin` when neither `OBSCURA_TIMEZONE` nor `TZ` is set, while explicit configuration is authoritative. Adding a synthetic timezone, inferring a zone from a detector, or changing BrowserScan behavior would make the profile less truthful.

### Evidence after

Not applicable to runtime behavior. The repeated explicit-Algiers control and the main/iframe/worker probes confirm that the existing implementation is coherent when its timezone input is explicit. No detector metric improvement is claimed from these local diagnostic runs.

### Remaining limitations

- A caller must set `OBSCURA_TIMEZONE` to the profile's intended IANA zone; the engine cannot safely infer it from locale, host settings, or detector output.
- CreepJS remains diagnostic because of the page error and synthetic unsupported surfaces such as WebGL/WebRTC.
- BrowserScan remains unavailable as a valid receipt because its result widgets are blank; the observed HTTP-0 failures are not evidence of a generic first-party module-loader failure.
