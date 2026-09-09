# Use a coherent Chrome 149 browser identity

Status: accepted

Obscura is not Chromium, so its browser identity must be kept coherent across the `wreq`/BoringSSL transport, HTTP client hints, JavaScript navigator surfaces, and CDP metadata. We use the newest profile supported by the pinned `wreq-util` release, Chrome 149 via `wreq-util` rc.14, and update every fallback identity with it; we do not claim unsupported WebGL/WebRTC capabilities or add detector-only values. Raw-direct BrowserScan and CreepJS runs are diagnostic evidence only, while the offline obstacle course remains the behavioral regression gate.

The stealth profile also uses the observed normal Chrome color preference (dark): `matchMedia`, CSS media-query selection, and `light-dark()` share one setting. Non-stealth runtimes retain the historical light default so this identity correction does not alter the engine's existing baseline lane.
