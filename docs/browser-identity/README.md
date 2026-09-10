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

- The current branch is `elias/chrome149-profile`; see git history for the
  latest identity and isolation commits.
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
  `obscura-net` passes 94/94. The broad serial fallback has exactly three
  child-frame lifecycle failures. The same three tests and assertions reproduce
  on merge-base `main` commit `727cc46`, so they are baseline failures rather
  than regressions from this identity/isolation work. The MCP target passes
  18/18 standalone and in the completed broad run.
- The offline obstacle course passes 33/33 after correcting its
  `observer-intersection` fixture to model real false-to-true crossings caused
  by scrolling. The engine was not changed to manufacture repeated callbacks.
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
`12f2276d4db865bc899f647a6423157d37a3c9115749308c6a56627e3298f1c1`.
Its post-batch state re-read is intentionally strict: it checks that earlier
contexts remain usable and retain their own state after later contexts are
created and disposed.

| Contexts | Navigation p50/p95 ms | State write/worker p50/p95 ms | Teardown ms | Peak RSS MB | After teardown MB | Failures |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 9.030 / 9.030 | 6.761 / 6.761 | 3.711 | 53.016 | 49.344 | 0 |
| 5 | 7.850 / 8.296 | 7.035 / 7.739 | 13.763 | 66.812 | 50.422 | 0 |
| 10 | 8.131 / 9.426 | 6.966 / 8.444 | 22.192 | 85.703 | 53.453 | 0 |
| 20 | 7.936 / 9.556 | 6.844 / 8.052 | 45.888 | 118.859 | 54.922 | 0 |

All state, proxy, and live-page reachability checks passed at every level. RSS
and teardown grow with the number of live contexts, while post-teardown RSS
returns toward the process baseline. This is deterministic local evidence for
the current lifecycle and isolation contract, not evidence about live-site
detectors or long-lived account behavior.

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
| Focused nextest | Changed crates and identity/isolation tests | Not run: locked install timed out reaching crates.io |
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
