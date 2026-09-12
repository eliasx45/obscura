# Obscura browser identity and instance isolation

Status: working investigation

Updated: 2026-09-10

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

- The current implementation is on the fork's main line and its follow-up
  integration branches; see git history for the latest identity and isolation
  commits.
- The renderer uses embedded Liberation, DejaVu, and Noto font assets and does
  not scan host system fonts. Common font metrics are therefore shared by
  instances using the same build. Page-provided web fonts remain page inputs.
- `BrowserContext` owns a cookie jar, storage directory, proxy setting, and HTTP
  client. These are the primary state-isolation boundaries to test.
- The only built-in preset is Chrome 149 on Windows. It is the default for
  every new context.
- The stealth transport, ordinary HTTP defaults, page JavaScript, and CDP
  metadata now use the same Windows Chrome 149 constants. Explicit low-level
  custom UA calls remain available for compatibility tests and are documented
  as potentially inconsistent overrides.
- Locale is pinned to `en-US`. Timezone is process-wide and is controlled by
  `OBSCURA_TIMEZONE`, `TZ`, or the intentional `Europe/Berlin` launch
  fallback. It is not a per-context identity field.
- The direct realm probe showed coherent timezone and `Function.toString`
  behavior across the main page, iframe, and worker when the process input was
  fixed.
- Each live CDP page now keeps one resident V8 runtime. A connection-level V8
  lock serializes access, so switching sessions does not suspend and rebuild a
  page or discard closures, listeners, workers, or pending async work.
- BrowserContext proxy routing is covered with one local proxy fixture per
  context. An explicit Playwright `proxyBypassList` is rejected because it is
  not implemented; an omitted proxy inherits the process default.
- The host-screen emulation failures were caused by read-only snapshot-backed
  bootstrap slots. Screen overrides now use private mutable state, and the
  focused emulation tests pass 10/10.
- The private-CA regression is fixed: serial release coverage for
  `obscura-net` passes 94/94. The three reported child-frame failures were
  caused by `a_rejected_child_frame_does_not_leave_js_references` leaking
  `OBSCURA_MAX_LIVE_FRAMES=0` into later tests in a reused fallback test
  process. A drop guard now restores the prior environment value; the complete
  child-frame file passes 11/11 with both serialized and default-thread
  fallback execution. The MCP target passes 18/18 standalone and in the
  completed broad run.
- `cargo-nextest` is installed. Historical authoritative release coverage
  before the upstream rebase was green: the full `render` run passed
  1,658/1,658 with 4 skipped; the full
  `render,stealth` run passes 1,668/1,668 with 4 skipped when bounded to two
  jobs. An unbounded stealth run showed intermittent loopback-fixture failures
  in MCP and screenshot-resource tests; the affected targets pass alone and
  in the bounded full run. This is recorded as runner/fixture timing
  sensitivity, with the exact scheduler/socket root cause initially unisolated,
  rather than as an implementation failure. Two subsequent unbounded reruns on
  this commit passed 1,668/1,668, so the issue was not reproduced in final
  verification.
- Latest revalidation on 2026-09-12 is not a merge-gate pass on the rebased
  fork tip: bounded `render` coverage was 1,710/1,711 with four local
  render-resource fixture failures across the rerun, and bounded
  `render,stealth` coverage was 1,719/1,722 with the two font/resource
  failures plus one `obscura-js` rendering-phase test. The phase-order test
  passes alone. The two font tests fail alone on this host, while other related
  fixtures pass; they were introduced with upstream render-transport commit
  `97ff86db` and are outside the Browser Use/identity diff. This remains an
  unresolved gate, not a “pre-existing” dismissal.
- The offline obstacle course passes 33/33 after correcting its
  `observer-intersection` fixture to model real false-to-true crossings caused
  by scrolling. The engine was not changed to manufacture repeated callbacks.
- CreepJS and BrowserScan runs in the current investigation are diagnostic
  controls only. Their page errors, blank widgets, and blocked third-party
  requests are not acceptance criteria or proof of a detector improvement.

### Browser Use compatibility audit

The official Browser Use client treats a browser profile primarily as a
state-and-configuration boundary. Its session/profile APIs expose persistent
or incognito storage, cookies and local storage state, proxy settings, request
headers, User-Agent, viewport, and profile paths. Parallel sessions require
separate profile or storage paths; the documentation does not make random
fingerprint variation a prerequisite for a usable session.

Sources reviewed on 2026-09-12:

- [BrowserSession](https://github.com/browser-use/browser-use/blob/main/browser_use/browser/session.py)
- [BrowserProfile](https://github.com/browser-use/browser-use/blob/main/browser_use/browser/profile.py)
- [Browser Use browser reference](https://github.com/browser-use/browser-use/blob/main/skills/open-source/references/browser.md)

The Obscura-side contract currently exercised for that workflow is:

- create and attach to a target, then receive post-navigation
  `Target.targetInfoChanged` metadata;
- use `DOM.getDocument` and `DOMSnapshot.captureSnapshot` to build an element
  index correlated by node identifiers;
- focus an element with `DOM.focus`, type with `Input.insertText`, and verify
  the result through `Runtime.evaluate`;
- keep storage, proxy, headers, and live page runtimes owned by the relevant
  BrowserContext.

`crates/obscura-cdp/tests/browser_use_contract.rs` covers the first three
steps with a deterministic local fixture. This audit does not define
session-varying identity. Clianta-specific requirements must be gathered
before deciding whether any identity properties should differ between
contexts. Do not cherry-pick or implement variation merely because Browser Use
supports configurable session settings.

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

Keep three questions separate:

- Default identity consistency: does one context report the same Windows
  Chrome 149 identity across its exposed surfaces?
- Session isolation: can mutable state or network configuration cross context
  boundaries?
- Session differences: which optional properties should vary between contexts?
  This remains open and must not be confused with either of the first two.

Incompatible identity overrides must fail clearly or be documented as
unsupported. Silent acceptance followed by a different identity is not
acceptable.

## Completed work

### 1. Default identity

The built-in identity now comes from shared constants used by ordinary HTTP,
stealth HTTP, page JavaScript, child realms, and CDP. Explicit UA and CDP
overrides remain compatibility surfaces and are not silently expanded into a
partial identity.

### 2. Instance isolation

The context-isolation tests assert that:

- cookies and local/session storage do not leak between contexts;
- persistent and incognito copies follow their documented rules;
- proxy and request callbacks stay attached to the owning context;
- a navigation or document replacement does not retain another context's DOM,
  realm, observer, or storage state;
- workers and iframes share their page's identity but not another page's state;
- later context creation and disposal do not make earlier live pages
  unevaluable or replace their runtimes;
- each context keeps its own Playwright proxy route and request headers after
  other contexts are created or disposed.

### 3. Deterministic benchmark

`benchmarks/context-isolation.py` runs the same release binary against one local
HTML fixture at 1, 5, 10, and 20 contexts. It records navigation, state,
worker/header/proxy checks, teardown, and RSS. The 2026-09-10 run used three
repetitions, a fixed 1280x720 viewport, the `render,stealth` release binary,
and SHA-256
`0ccf690bb0a14d1002dc3ba1b3812605e05076d8bb8f55db15c88863aa1b2618`.
Its post-batch state re-read is intentionally strict: it checks that earlier
contexts remain usable and retain their own state after later contexts are
created and disposed.

| Contexts | Navigation p50/p95 ms | State write/worker p50/p95 ms | Teardown ms | Peak RSS MB | After teardown MB | Failures |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 7.847 / 7.847 | 6.492 / 6.492 | 2.079 | 55.750 | 52.078 | 0 |
| 5 | 7.245 / 8.490 | 6.707 / 7.143 | 10.652 | 69.250 | 52.859 | 0 |
| 10 | 7.181 / 7.869 | 6.540 / 6.946 | 19.141 | 87.656 | 55.406 | 0 |
| 20 | 7.313 / 8.401 | 6.661 / 7.182 | 45.481 | 121.812 | 57.953 | 0 |

All state, proxy, and live-page reachability checks passed at every level. RSS
and teardown grow with the number of live contexts, while post-teardown RSS
returns toward the process baseline. This is deterministic local evidence for
the current lifecycle and isolation contract, not evidence about live-site
detectors or long-lived account behavior.

Fresh validation on 2026-09-12 used the current fork branch, the exact
`render,stealth` release build, and three repetitions. The binary SHA-256 was
`be9af430442f851b84f7826f313fcdd199a399b101cf9fbbebbeee44585d2b45`.

| Contexts | Navigation p50/p95 ms | State write/worker p50/p95 ms | Teardown ms | Peak RSS MB | Failures |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 13.7 / 13.7 | 7.0 / 7.0 | 2.3 | 53.2 | 0 |
| 5 | 7.8 / 8.4 | 6.3 / 7.9 | 10.6 | 66.6 | 0 |
| 10 | 7.7 / 8.4 | 6.4 / 6.8 | 19.7 | 84.7 | 0 |
| 20 | 7.4 / 8.3 | 6.4 / 6.7 | 35.2 | 118.4 | 0 |

This rerun also passed the strict post-batch re-read and teardown checks at
every level.

### 4. Use external sites only as diagnostic controls

External-site runs are diagnostic controls. Every run records the binary hash,
launch mode, configuration, errors, and critical rows. A blank widget, HTTP 0,
or headline score is not a pass/fail gate by itself.

Do not use live account activity as the benchmark. If an approved product
workflow later needs qualification, test it separately with explicit authority,
no unsolicited actions, and no detector-page instrumentation.

## Test matrix

| Test | Purpose | Acceptance |
| --- | --- | --- |
| Identity contract fixture | Main, iframe, worker, HTTP, and CDP agreement | One Windows Chrome 149 identity, or a clear unsupported error |
| Context isolation fixture | Cookies, storage, DOM, workers, callbacks | No cross-context leakage |
| Same-build font/render fixture | Shared immutable renderer behavior | Stable metrics under identical inputs |
| Obstacle course | Broad offline capability regression | 33/33 |
| Release configuration builds | Render, stealth, no-render, no-render stealth | All supported configurations build |
| Focused nextest | Changed crates and identity/isolation tests | Pass: child frames 11/11, CDP ownership 7/7, identity/isolation 2/2, stealth transport 1/1, gzip transport 1/1 |
| Browser Use CDP contract | Target discovery, navigation metadata, DOM snapshot, focus, input, evaluation | 1/1 local deterministic fixture |
| Full release nextest | Complete render suite | Historical 1,658/1,658 pass; latest rebased-tip run was 1,710/1,711 with render-resource failures |
| Full release nextest with stealth | Complete render + stealth suite | Historical 1,668/1,668 pass; latest rebased-tip run was 1,719/1,722 with two font/resource failures and one phase-order failure |
| Bounded multi-context benchmark | Lightweight concurrency and lifecycle | 1/5/10/20 pass with zero state, proxy, and reachability failures |
| CreepJS / BrowserScan | Diagnostic surface inspection | Row-level evidence only, never headline-only acceptance |

## Evidence record

For each reusable run, record a compact receipt with:

- date, commit, binary hash, host and architecture;
- feature configuration and exact launch arguments;
- built-in identity, timezone, locale, viewport, and storage/proxy mode;
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

1. Which properties, if any, should intentionally vary between sessions?
   Candidate inputs include timezone, locale, viewport, fingerprint seed, and
   network configuration. This is separate from keeping each session coherent.
2. Should a future custom-UA API also update client hints and platform, or
   remain an explicit low-level override with the current warning?
3. Which additional session-varying properties, if any, should be proposed
   after the default identity and isolation contracts remain stable?
