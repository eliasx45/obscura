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
`0ccf690bb0a14d1002dc3ba1b3812605e05076d8bb8f55db15c88863aa1b2618`)
passed the immediate and post-batch checks for every level:

| Contexts | Navigation p50/p95 ms | State write/worker p50/p95 ms | Teardown ms | Peak RSS MB | After teardown MB | Failures |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 7.847 / 7.847 | 6.492 / 6.492 | 2.079 | 55.750 | 52.078 | 0 |
| 5 | 7.245 / 8.490 | 6.707 / 7.143 | 10.652 | 69.250 | 52.859 | 0 |
| 10 | 7.181 / 7.869 | 6.540 / 6.946 | 19.141 | 87.656 | 55.406 | 0 |
| 20 | 7.313 / 8.401 | 6.661 / 7.182 | 45.481 | 121.812 | 57.953 | 0 |

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

The Browser Use CDP contract test passes 1/1 on the current integration
branch. The full `obscura-cdp` render package passes 206/206 with 3 skipped,
including the contract test. This confirms the client workflow without making
session-varying identity part of the isolation decision.

The Clianta audit shows that its current Browser Use-style layer leases an
existing profile session and sends a bounded CDP command set through its own
gateway. It does not require Browser Use Cloud fingerprint variation or a new
Obscura profile pool. Future changes should therefore target observed protocol,
context-lifecycle, or isolation gaps only.

Clianta's separate Veil launcher does currently bind profiles to persistent
user-data, proxy routes, and stored fingerprint data, with runtime UA/platform
verification. If Obscura later becomes that launcher's backend, profile-bound
identity support must be designed explicitly; it is not implied by Browser
Use's session/profile API and is outside this decision.

The latest workspace-wide revalidation on 2026-09-12 is not a merge-gate pass:
bounded render nextest reported 1,710/1,711 and render-plus-stealth reported
1,719/1,722. The failures are two upstream render-resource/font fixtures,
plus one stealth-run rendering-phase fixture; the phase-order fixture passes
alone. They remain explicitly unresolved rather than being called
pre-existing.

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
