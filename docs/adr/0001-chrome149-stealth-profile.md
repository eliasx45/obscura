# Use a coherent Chrome 149 browser identity

Status: accepted

Obscura is not Chromium, so its browser identity must be kept coherent across the `wreq`/BoringSSL transport, HTTP client hints, JavaScript navigator surfaces, and CDP metadata. We use the newest profile supported by the pinned `wreq-util` release, Chrome 149 via `wreq-util` rc.14, and update every fallback identity with it; we do not claim unsupported WebGL/WebRTC capabilities or add detector-only values. Raw-direct BrowserScan and CreepJS runs are diagnostic evidence only, while the offline obstacle course remains the behavioral regression gate.
