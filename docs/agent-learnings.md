# Agent learnings

This is the short, durable record of non-obvious Obscura behavior and
investigation rules. Read it before changing identity, timezone, UA/CDP
overrides, context isolation, shared renderer resources, or the related tests.
Update the general principle here when new evidence changes it. Do not turn
this into a log of every command or a copy of the living test plan.

## Identity means what the page is told

The host OS is the machine running Obscura. A reported identity is the browser
characteristics exposed to a website. Saying “Windows Chrome 149” in this
project means the latter: one built-in reported identity, not that the process
has become a Windows kernel or has acquired Windows-native fonts and rendering.

The default contract is one coherent Windows Chrome 149 identity across the
surfaces Obscura controls: HTTP, JavaScript, iframes, workers, and CDP. Keep
these three questions separate:

1. Does one default context describe itself consistently?
2. Does mutable state stay isolated between contexts?
3. Which properties, if any, should intentionally vary between contexts?

The first two are current requirements. The third is still open. Do not add
identity variation as a substitute for isolation.

## Timezone is process configuration, not a Windows profile field

The CLI chooses the timezone in this order: `OBSCURA_TIMEZONE`, then `TZ`, then
the process fallback `Europe/Berlin` when both are unset. This fallback keeps
the process's `Date` and `Intl` behavior deterministic and internally
coherent; it is not evidence that the browser is Windows, and it is not a
per-context profile setting. Treat timezone as process-wide until the API and
tests explicitly support a different scope.

Native Windows needs an explicit ICU default for that CLI contract. Bundled
V8 137's ICU host-timezone detection uses `uprv_detectWindowsTimeZone()` on
Windows rather than its Unix `TZ` lookup. Setting the CLI's `TZ` fallback is
therefore insufficient. After startup environment and V8 flags are configured,
the CLI initializes bundled ICU and sets its native default before the first
isolate. The binding uses the bundled ICU 74 ABI; verify it when upgrading V8.
Changing the process timezone after platform startup is rejected. Do not change the host timezone,
rewrite only the JavaScript timezone label, or relax the benchmark expectation
to hide it. A native fix must keep `Date`, `Intl`, offsets, and DST coherent.

When investigating an identity mismatch, first record the actual environment
inputs and launch mode. Never infer a reported OS or timezone from the host
machine alone.

## Why custom UA and CDP overrides remain

Custom User-Agent and CDP metadata overrides are deliberate low-level escape
hatches for compatibility, embedding, diagnostics, and callers that need to
test protocol override behavior. They are not additional built-in profiles and
they are not promised to rewrite every related identity surface.

Preserving them avoids breaking existing callers and keeps the override API
useful. An explicit override can therefore make the reported identity
internally inconsistent. That is an intentional, documented tradeoff of using
the escape hatch; the default path remains the authoritative coherent preset.
Tests for the default must not silently replace this contract with “all
overrides are coherent.” Tests for overrides should verify their documented
scope and explicitly acknowledge the possible inconsistency.

## Immutable sharing is compatible with isolation

Pages can share read-only engine resources from the same build: Rust/V8 code,
DOM and renderer implementation, embedded font assets, and other immutable
tables. This keeps many contexts lightweight. Sharing those resources means
that identical inputs use the same implementation and font metrics; it does
not mean contexts share cookies, storage, workers, document globals, request
headers, proxy routing, or other mutable session state.

Do not duplicate the engine merely to make contexts look different. First prove
that mutable state and network configuration are isolated. Treat deliberately
varying identity properties as a separate product decision with its own
evidence.

## Test-process environment is shared state

`cargo test` may run several tests inside one test-binary process, so environment
variables changed by a test can affect later tests. `cargo nextest` isolates
tests in separate processes and is the authoritative runner for Obscura, but
fallback tests must still restore any process-global variable they modify. Use
a drop guard that restores the previous value, including when the test panics.

If a failure appears only in a broad fallback run, reproduce the test alone and
then in its full test file before changing production code or expectations.
Check for leaked environment, shared ports, global singletons, and test-order
dependence. A clean merge-base run is required before calling a failure
pre-existing.

When a local-fixture test fails only in a broad nextest run, rerun the same
feature set with bounded nextest jobs before changing browser behavior.
Loopback fixtures and child-process tests can contend for CPU, sockets, and
startup time. Record both results; a bounded passing run is evidence of runner
or fixture timing sensitivity, not proof that the exact scheduler/socket root
cause has been isolated.

## Evidence before claims

Local TCP fixtures must set the accepted stream's I/O mode explicitly. On
Windows, a socket accepted from a nonblocking listener can still be
nonblocking: reading immediately can return `WouldBlock` before request bytes
arrive. A handler written for blocking reads must call `set_nonblocking(false)`
and set a bounded read timeout. Otherwise resource-loading tests can report
missing responses even though the browser correctly connected and sent them.

When resource loads can complete during evaluation or intermediate queue
steps, the last drain's return value counts only the remaining work. Verify
cumulative successful responses and an empty pending queue to assert that
every requested resource loaded; keep the concurrency ceiling assertion.

HTTP response fixtures should consume the request headers before closing the
connection, even when they ignore their contents. Closing with unread input
can reset a small response on Windows. Observer tests should advance to the
next state after delivery rather than assuming several short wall-clock
timers will run in separate rendering turns under CPU load.

Record the exact commit, feature flags, runner, test filter, environment
inputs, and whether the result came from a clean merge-base. A passing focused
test does not prove a broad suite passes, and a broad fallback failure does not
prove a product regression. Keep implementation failures, fixture failures,
runner limitations, and infrastructure failures as separate categories.

## Browser Use compatibility is a session/CDP contract

Browser Use's profile model is primarily about persistent or isolated state
and configurable session settings such as storage, proxy, headers, User-Agent,
and viewport. It does not by itself establish a requirement for randomized
fingerprints. Validate the CDP workflow and Clianta's actual needs first; do
not introduce session-varying identity as a substitute for isolation or as an
assumption imported from the client library.

The current Clianta integration is not Browser Use Cloud's browser runtime.
`@clianta/browser-use` is a policy and session layer: it leases an existing
Clianta Browser Profile session and sends a deliberately bounded CDP operation
set through `BrowserGateway`. Therefore an Obscura change must be justified by
an observed CDP, context-lifecycle, or network-isolation requirement from that
adapter. Browser Use Cloud claims about hosted profiles, fingerprint
variation, or fleet behavior are not implementation requirements for Obscura.

Do not confuse that adapter boundary with Clianta's existing Veil launcher.
The launcher currently binds a connected profile to persistent user-data,
proxy-route, and stored `fingerprint_json` inputs, then verifies runtime UA and
platform signals against the stored profile. If Obscura is ever made a Veil
backend, profile-bound identity support will need its own explicit design and
coherence tests. Browser Use's configurable session fields do not settle that
design, and they are not permission to add a random identity pool.
