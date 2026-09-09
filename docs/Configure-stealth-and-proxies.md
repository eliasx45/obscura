## Stealth mode

```bash
obscura fetch https://example.com --stealth
obscura serve --stealth
obscura scrape url1 url2 --stealth
obscura mcp --stealth
```

`--stealth` is a global flag, so it works before or after the subcommand and applies to `fetch`, `serve`, `scrape`, and `mcp`. In a `scrape` run each worker inherits it.

What `--stealth` changes:

- Uses the wreq HTTP client with browser-matching TLS fingerprints (ClientHello, ALPN, cipher order).
- Loads a tracker blocklist that drops requests to known analytics and fingerprinting endpoints.
- Bundles webpki roots instead of relying on the system store.

Requires a build that includes the stealth feature. Use a `-stealth` archive
with rendering or a `-no-render-stealth` archive without it. To build the
rendering variant yourself:

```bash
cargo build --release -p obscura-cli --bins --features render,stealth
```

Omit rendering with `cargo build --release -p obscura-cli --bins --no-default-features --features stealth`.

## What stealth handles

- Basic bot detection that checks TLS fingerprint or User-Agent.
- Sites that rely on third-party analytics being reachable.

## What stealth does not handle

- Cloudflare interactive challenges.
- Datadome and Akamai bot manager active challenges.
- CAPTCHAs.
- IP-based rate limiting (use proxies).

## Proxies

HTTP proxy:

```bash
obscura fetch https://example.com --proxy http://proxy.example.com:8080
obscura serve --proxy http://proxy.example.com:8080
```

With auth:

```bash
obscura fetch https://example.com --proxy http://user:pass@proxy.example.com:8080
```

SOCKS5:

```bash
obscura fetch https://example.com --proxy socks5://proxy.example.com:1080
```

## Custom User-Agent

```bash
obscura fetch https://example.com --user-agent "Mozilla/5.0 (...) ..."
obscura serve --user-agent "Mozilla/5.0 (...) ..."
```

The supported default is Chrome 149 on Windows, regardless of the host OS.
`--user-agent` is a low-level override for compatibility tests. It can make
the reported UA disagree with the supported Windows identity, so it is not
part of the normal identity contract. In stealth mode the transport and page
identity continue to use the supported Windows values.

## Browser profile, timezone, and geolocation

The current supported identity is one stable profile: Chrome 149 on Windows.
It keeps `navigator.platform`, `navigator.userAgentData`, HTTP defaults, and
CDP browser metadata aligned. macOS and Linux profiles are not supported yet.
The engine has no GPU renderer: `canvas.getContext('webgl')` returns `null`.

The legacy profile selectors remain accepted but are ignored while the
identity surface is being unified:

`OBSCURA_PROFILE` and `OBSCURA_ROTATE_PROFILE` are reserved for a future
version in which every identity surface, including transport and CDP, is
selected from the same profile.

Timezone is driven by the process zone so `Date` (`getTimezoneOffset`, `toString`) and `Intl.DateTimeFormat` report the same region. Default is `Europe/Berlin`; set it to match the exit IP:

```bash
OBSCURA_TIMEZONE=America/New_York obscura serve
```

`navigator.geolocation` reports configurable coordinates. Set them as `lat,lon` and keep them consistent with the timezone and proxy region:

```bash
OBSCURA_GEOLOCATION="40.7128,-74.0060" obscura serve
```

Keep timezone and geolocation aligned with the intended network region. See
[Environment variables](Environment-variables.md) for the full list.

## Combine

```bash
obscura serve \
  --stealth \
  --proxy http://user:pass@proxy.example.com:8080
```
