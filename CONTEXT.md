# Obscura browser identity and isolation

This glossary defines the terms used when reasoning about reported browser identity and mutable session state.

## Language

**Reported identity**:
The browser characteristics exposed to a website, such as User-Agent, platform, client hints, timezone, and related JavaScript/CDP values. It is not the host operating system.
_Avoid_: Host OS, profile as a separate browser

**BrowserContext**:
An isolated mutable-state container that may own multiple pages and websites.
_Avoid_: Tab, account, website session

**Page**:
A tab-like document owner with its main document, child frames, workers, and live JavaScript runtime.
_Avoid_: BrowserContext

**Default identity consistency**:
Whether one built-in reported identity agrees across all supported browser surfaces.
_Avoid_: Uniqueness between sessions

**Session isolation**:
Whether mutable state and network configuration stay inside their owning BrowserContext.
_Avoid_: Different identity

**Session-varying properties**:
Properties deliberately chosen to differ between BrowserContexts. These are a separate product decision from identity consistency and isolation.
_Avoid_: Treating identity variation as a prerequisite for isolation
