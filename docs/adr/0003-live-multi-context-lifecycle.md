# Keep live page runtimes resident across context switching

Status: accepted

Obscura keeps one separate V8 runtime resident for each live Page on a CDP connection and serializes access with the connection-level V8 lock. CDP session routing and context creation must not suspend and reconstruct another page, because that loses closures, DOM listeners, workers, pending async work, and other live state. BrowserContexts may also carry an explicit proxy from Playwright's `Target.createBrowserContext.proxyServer`; unsupported bypass-list options return an error instead of being ignored, while a missing proxy inherits the process default.

This is a lightweight logical multi-context design, not one OS process per page.
Context disposal drops only that context's pages and state; other live contexts
remain reachable. The focused ownership regression and the deterministic
1/5/10/20 context benchmark pass with earlier pages still evaluable after later
contexts are created and disposed.
