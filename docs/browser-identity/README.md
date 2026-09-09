# Obscura browser identity and instance isolation

Status: working investigation

Updated: 2026-09-09

This is the living engineering record for browser identity, per-context state
isolation, compatibility, and performance. Use ADRs for decisions that have
settled. Keep this document for hypotheses, measurements, open questions, and
the current test matrix.

This is an Obscura document, not a Clianta document. It is about truthful,
internally consistent browser behavior and safe session isolation. It is not a
plan for making accounts appear unrelated or bypassing platform safeguards.

## Mental model

| Term | Meaning |
| --- | --- |
| Host | The real OS and machine running the Obscura binary. A profile does not change it. |
| Engine | One Rust/V8/DOM/rendering implementation shared by pages on that build. |
| Browser context | An isolated browser state container with its own cookies, storage, and HTTP client. |
| Page and realm | A document plus its main, iframe, and worker JavaScript realms. |
| Profile | A reported browser identity, such as UA, platform, and client hints. |

A profile is a reported identity, not a separate Windows, macOS, or Linux
browser engine. A complete profile must agree across JavaScript, HTTP, TLS,
CDP, and child realms wherever those surfaces are exposed.

## Current confirmed state

- The current branch is `elias/chrome149-profile` at `f5daac6`.
- The renderer uses embedded Liberation, DejaVu, and Noto font assets and does
  not scan host system fonts. Common font metrics are therefore shared by
  instances using the same build. Page-provided web fonts remain page inputs.
- `BrowserContext` owns a cookie jar, storage directory, proxy setting, and HTTP
  client. These are the primary state-isolation boundaries to test.
- The active profile table now contains one preset: Chrome 149 on Windows.
  Legacy profile and rotation environment variables are ignored until
  selectable identities can be wired end to end.
- The stealth transport, ordinary HTTP defaults, page JavaScript, and CDP
  metadata now use the same Windows Chrome 149 constants. Explicit low-level
  custom UA calls remain available for compatibility tests and are outside
  this supported identity contract.
- Locale is pinned to `en-US`. Timezone is process-wide and is controlled by
  `OBSCURA_TIMEZONE`, `TZ`, or the current Europe/Berlin fallback. It is not a
  per-context profile field.
- The direct realm probe showed coherent timezone and `Function.toString`
  behavior across the main page, iframe, and worker when the process input was
  fixed.
- The offline obstacle course currently reports 32/33. The existing failure is
  `observer-intersection`, expected `io:50` but received an empty value.
- CreepJS and BrowserScan runs in the current investigation are diagnostic
  controls only. Their page errors, blank widgets, and blocked third-party
  requests are not acceptance criteria or proof of a detector improvement.

## Target contract

For the current product direction, use one explicit default identity:

```text
Chrome 149 + Windows
```

The implementation should make the same identity authoritative for:

- request User-Agent and client hints;
- TLS emulation when stealth is enabled;
- `navigator` and `userAgentData` in the main page, iframes, and workers;
- CDP browser metadata;
- related browser identity values that are intentionally profile-scoped.

The engine foundation can remain shared. Fonts, renderer code, and other
immutable resources do not need to be duplicated per instance. Isolation means
that mutable state does not cross contexts, not that every instance needs a
different renderer binary.

Incompatible identity overrides must fail clearly or be documented as
unsupported. Silent acceptance followed by a different identity is not
acceptable.

## Work plan

### 1. Establish one identity source of truth

Resolve the current duplication between `profiles.rs`, stealth constants,
HTTP defaults, and CDP metadata. The first implementation should be small:

1. Make Windows Chrome 149 the active default identity.
2. Remove or disable macOS profile activation while the product contract is
   Windows-only.
3. Decide whether incompatible custom UA settings should be rejected at the
   CLI boundary; the current compatibility API still permits explicit UA
   overrides and documents that they can leave the supported identity.
4. Add an end-to-end identity test that compares context, page, iframe, worker,
   HTTP, and CDP values.

Selectable identities can return later only when the transport and all exposed
surfaces are wired to the same selected profile.

### 2. Prove instance isolation

Use two or more contexts in one process and assert that:

- cookies and local/session storage do not leak between contexts;
- persistent and incognito copies follow their documented rules;
- proxy and request callbacks stay attached to the owning context;
- a navigation or document replacement does not retain another context's DOM,
  realm, observer, or storage state;
- workers and iframes share their page's identity but not another page's state.

### 3. Build a lightweight multi-instance performance harness

Use deterministic local fixtures rather than live social accounts. Run the same
release binary with 1, 5, 10, and 20 contexts and record:

- startup and first-navigation latency, p50 and p95;
- steady-state navigation/evaluation latency;
- resident memory and CPU;
- context creation/teardown failures;
- cross-context isolation failures;
- hangs, watchdog terminations, and resource growth during a bounded soak.

The harness should vary only the number of contexts. Keep URL, viewport, build
features, fonts, settle time, and fixture data fixed. This measures whether the
browser stays light without turning a detector score into a product target.

### 4. Use external sites only as diagnostic controls

CreepJS and BrowserScan may help identify a surface regression after local
tests pass. Every run records the binary hash, launch mode, configuration,
errors, and critical rows. A blank widget, HTTP 0, or headline score is not a
pass/fail gate by itself.

Do not use live account activity as the benchmark. If an approved product
workflow later needs qualification, test it separately with explicit authority,
no unsolicited actions, and no detector-page instrumentation.

## Test matrix

| Test | Purpose | Acceptance |
| --- | --- | --- |
| Identity contract fixture | Main, iframe, worker, HTTP, and CDP agreement | One Windows Chrome 149 identity, or a clear unsupported error |
| Context isolation fixture | Cookies, storage, DOM, workers, callbacks | No cross-context leakage |
| Same-build font/render fixture | Shared immutable renderer behavior | Stable metrics under identical inputs |
| Obstacle course | Broad offline capability regression | 33/33; track `observer-intersection` separately until fixed |
| Release configuration builds | Render, stealth, no-render, no-render stealth | All supported configurations build |
| Focused nextest | Changed crates and identity/isolation tests | Pass in process-isolated test runner |
| Bounded multi-context soak | Lightweight concurrency and lifecycle | No hangs, leaks, or isolation failures |
| CreepJS / BrowserScan | Diagnostic surface inspection | Row-level evidence only, never headline-only acceptance |

## Evidence record

For each reusable run, record a compact receipt with:

- date, commit, binary hash, host and architecture;
- feature configuration and exact launch arguments;
- profile, timezone, locale, viewport, and storage/proxy mode;
- context count and fixture or URL;
- latency, RSS, CPU, errors, and pass/fail rows;
- clean versus instrumented label for external diagnostics;
- decision, remaining uncertainty, and next action.

Never store proxy credentials, authentication state, or generated screenshots in
this living document. Keep large generated outputs outside the repository and
link only to sanitized, durable evidence when it is actually needed.

## Document policy

- This file is the working index and test plan.
- `docs/adr/` contains durable decisions and their rationale.
- Real-run receipts belong beside the relevant investigation only after they
  change a decision or provide reusable evidence.
- The companion `obscura-benchmark` repository remains the owner of the
  obstacle-course fixtures and runner; do not copy its generated output here.

## Open questions

1. Should the Windows Chrome 149 identity be the only profile in the first
   supported release, or should Windows-only older Chrome versions remain as
   explicit compatibility presets?
2. Should incompatible `OBSCURA_PROFILE`, rotation, and custom UA settings
   fail at CLI startup or at browser-context creation?
3. Should CDP browser metadata be fixed to the product identity or be attached
   to a selected context once profile selection exists?
4. What is the smallest deterministic fixture that reproduces the current
   `observer-intersection` failure without involving detector sites?
