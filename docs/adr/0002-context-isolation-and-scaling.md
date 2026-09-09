# Keep context state isolated while sharing the default identity

Status: accepted

Date: 2026-09-09

## Decision

Every new browser context uses the one built-in Windows Chrome 149 identity.
Identity consistency is checked within a context across HTTP, page JavaScript,
iframes, workers, and CDP. Contexts may share immutable engine and font
resources, but cookies, web storage, document state, workers, HTTP headers,
proxy configuration, and persistence state remain context-owned.

Explicit custom User-Agent and CDP overrides remain available for compatibility.
They are low-level overrides and may intentionally make a context internally
inconsistent; they do not select another built-in identity.

Which properties should differ between sessions is intentionally unresolved.
That question is separate from both per-context consistency and isolation.

## Evidence

`crates/obscura-browser/tests/identity_consistency.rs` verifies the default
identity in the main page, a same-origin iframe, a worker, and captured HTTP
requests. CDP `Browser.getVersion` and `Network.setUserAgentOverride` retain
their explicit coverage. `context_isolation.rs` verifies cookies, local and
session storage, document state, workers, proxy/private-network settings,
extra headers, copied versus incognito jars, and disk cookie persistence. The
initial storage write is asserted before checking for leakage.

The deterministic local benchmark is
`benchmarks/context-isolation.py`. On 2026-09-09, three repetitions with a
fixed 1280x720 fixture and the `render,stealth` binary (SHA-256
`fb69c1e6d324935be272ae973b837785f3f3a15bd1b772b082b75162d4242db8`)
completed all immediate write/worker/header checks:

The immediate write/worker/header checks passed. The stricter post-batch
re-read found that pages in 5+ live contexts could no longer be evaluated:

| Contexts | Navigation p50/p95 ms | State write/worker p50/p95 ms | Teardown ms | Peak RSS MB | After teardown MB | Post-batch failures |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 7.021 / 7.021 | 6.490 / 6.490 | 1.894 | 33.719 | 30.031 | 0 |
| 5 | 7.429 / 7.810 | 6.700 / 7.094 | 8.994 | 33.766 | 30.078 | 15 |
| 10 | 7.135 / 8.010 | 6.377 / 6.943 | 15.755 | 33.859 | 30.172 | 30 |
| 20 | 7.643 / 8.887 | 6.716 / 7.407 | 33.481 | 33.906 | 30.219 | 60 |

The failure is `Cannot read properties of undefined (reading 'evaluate')` on a
later page evaluation; navigating the page recreates its realm. This is a
multi-context lifecycle limitation, so the 5/10/20 runs do not establish a
state-leakage guarantee. These numbers describe one local fixture and are not
evidence about live-site detectors or long-lived account behavior.

The required `cargo-nextest` runner was not available and its locked install
could not reach crates.io. The serial `cargo test --release --features render`
fallback passed the changed identity, isolation, and transport coverage, plus
94/94 `obscura-net` tests after the private-CA fix. It still exposes two
host-screen emulation assertions and three child-frame lifecycle assertions;
those source files are outside this change. The one MCP navigation failure
occurred only in the broad run; the complete target passes 18/18 standalone
with one test thread. The offline obstacle course remains 32/33 for the documented
IntersectionObserver fixture mismatch.

## Consequences

- Adding a new built-in identity requires wiring and testing every exposed
  surface before it can be supported.
- Different contexts can be lightweight without pretending to be separate
  browser engines or requiring duplicate font/render resources.
- Active CDP multi-context lifecycle remains an open implementation issue. Do
  not treat the 5/10/20 measurements as production readiness evidence until
  post-batch page evaluation succeeds.
- Session-varying properties need a separate proposal with explicit consistency
  rules and benchmark coverage.
