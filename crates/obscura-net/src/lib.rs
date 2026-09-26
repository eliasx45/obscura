pub mod client;
pub mod cookies;
pub mod encoding;
pub mod interceptor;
pub mod robots;
pub mod blocklist;

/// The only built-in browser identity. Keep transport, JavaScript, and CDP
/// fallbacks on these values.
pub const BROWSER_USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/149.0.0.0 Safari/537.36";
pub const BROWSER_NAVIGATOR_PLATFORM: &str = "Win32";
pub const BROWSER_UA_PLATFORM: &str = "Windows";
pub const BROWSER_UA_PLATFORM_VERSION: &str = "15.0.0";

/// Return the JavaScript platform identity that belongs with a User-Agent.
///
/// The stealth transport currently has one supported identity (Chrome 149 on
/// Windows). Callers that use a custom UA on the ordinary transport still
/// need navigator.platform and userAgentData.platform to describe the same
/// OS family as the request headers.
pub fn browser_platform_for_user_agent(ua: &str) -> (&'static str, &'static str, &'static str) {
    if ua.contains("Windows NT") {
        (BROWSER_NAVIGATOR_PLATFORM, BROWSER_UA_PLATFORM, BROWSER_UA_PLATFORM_VERSION)
    } else if ua.contains("iPhone") || ua.contains("iPad") {
        ("iPhone", "iOS", "0.0.0")
    } else if ua.contains("Android") {
        ("Linux armv8l", "Android", "0.0.0")
    } else if ua.contains("Macintosh") {
        ("MacIntel", "macOS", "10.15.7")
    } else if ua.contains("Linux") {
        ("Linux x86_64", "Linux", "0.0.0")
    } else {
        // Unknown UA strings retain the existing browser defaults rather than
        // inventing a platform identity that may be less coherent.
        (BROWSER_NAVIGATOR_PLATFORM, BROWSER_UA_PLATFORM, BROWSER_UA_PLATFORM_VERSION)
    }
}

#[cfg(feature = "stealth")]
pub mod wreq_client;

pub use client::{
    env_allows_private_network, is_forbidden_ip, CallbackRegistry, ObscuraHttpClient,
    ObscuraNetError, RequestCallback, RequestCredentials, RequestInfo, RequestMode,
    ResourceRequest, ResourceType, Response, ResponseCallback, SsrfGuardResolver,
};
pub use cookies::{canonical_domain, default_cookie_path, CookieInfo, CookieJar};
pub use encoding::{
    decode_non_html, decode_response, decode_response_with_name, decode_with_label, label_name,
    url_encode_query,
};
pub use robots::RobotsCache;
pub use blocklist::is_blocked as is_tracker_blocked;
#[cfg(feature = "stealth")]
pub use wreq_client::{
    StealthHttpClient, STEALTH_NAVIGATOR_PLATFORM, STEALTH_UA_PLATFORM,
    STEALTH_UA_PLATFORM_VERSION, STEALTH_USER_AGENT,
};
