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
`benchmarks/context-isolation.py`. On 2026-09-10, three repetitions with a
fixed 1280x720 fixture and the `render,stealth` binary (SHA-256
`12f2276d4db865bc899f647a6423157d37a3c9115749308c6a56627e3298f1c1`)
passed the immediate and post-batch checks for every level:

| Contexts | Navigation p50/p95 ms | State write/worker p50/p95 ms | Teardown ms | Peak RSS MB | After teardown MB | Failures |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 9.093 / 9.093 | 6.709 / 6.709 | 3.562 | 54.000 | 50.328 | 0 |
| 5 | 7.606 / 8.918 | 6.743 / 7.062 | 12.989 | 67.250 | 50.859 | 0 |
| 10 | 8.044 / 8.789 | 6.540 / 7.166 | 23.190 | 85.406 | 53.156 | 0 |
| 20 | 7.898 / 8.369 | 6.593 / 7.504 | 46.485 | 120.297 | 56.359 | 0 |

Every state, proxy, and page-reachability check passed. These numbers describe
one local fixture and are not evidence about live-site detectors or long-lived
account behavior.

The lifecycle regression test
`crates/obscura-cdp/tests/execution_context_ownership.rs` keeps multiple pages
alive while creating and disposing later contexts, then re-evaluates every
earlier page. It covers document state, cookies, local/session storage,
workers, headers, proxy routing, and teardown.

`cargo-nextest` is installed and authoritative. The full release `render` run
passes 1,658/1,658 with 4 skipped. The full `render,stealth` run passes
1,668/1,668 with 4 skipped at `-j 2`; an unbounded stealth run showed
intermittent loopback-fixture timing failures, while the affected MCP and
screenshot targets pass in isolation and in the bounded full run. The exact
scheduler/socket root cause is not isolated, so this remains a runner/fixture
uncertainty rather than an implementation failure. Two subsequent unbounded
reruns on this commit also passed 1,668/1,668, so the issue was not reproduced
in final verification. The child-frame file passes
11/11 after its test restores the process-global frame-cap variable. The
focused host-screen tests pass 10/10, the MCP target passes 18/18, and
`obscura-net` passes 94/94 after the private-CA fix. The obstacle course passes
33/33 after its fixture was corrected to model real IntersectionObserver
crossings.

## Consequences

- Adding a new built-in identity requires wiring and testing every exposed
  surface before it can be supported.
- Different contexts can be lightweight without pretending to be separate
  browser engines or requiring duplicate font/render resources.
- Active CDP multi-context lifecycle is covered by resident page runtimes and
  the post-batch benchmark. The benchmark remains local evidence, not a
  production or detector-readiness claim.
- Per-context proxies are supported through Playwright's
  `proxyServer`; unsupported bypass lists fail explicitly rather than being
  silently ignored.
- Session-varying properties need a separate proposal with explicit consistency
  rules and benchmark coverage.
