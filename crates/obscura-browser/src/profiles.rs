pub struct BrowserProfile {
    pub user_agent: &'static str,
    pub platform: &'static str,
    pub ua_platform: &'static str,
    pub ua_platform_version: &'static str,
}

pub static PROFILES: &[BrowserProfile] = &[BrowserProfile {
    user_agent: obscura_net::BROWSER_USER_AGENT,
    platform: obscura_net::BROWSER_NAVIGATOR_PLATFORM,
    ua_platform: obscura_net::BROWSER_UA_PLATFORM,
    ua_platform_version: obscura_net::BROWSER_UA_PLATFORM_VERSION,
}];

pub fn random_profile() -> &'static BrowserProfile {
    let idx = (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos() as usize)
        % PROFILES.len();
    &PROFILES[idx]
}

/// Pick the only supported profile for a new browser context.
///
/// The old profile and rotation environment variables remain harmlessly
/// ignored for compatibility. Selectable identities can return only when all
/// exposed identity surfaces use the same selected profile.
pub fn select_profile() -> &'static BrowserProfile {
    &PROFILES[0]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supported_profile_is_windows_chrome149() {
        assert_eq!(PROFILES.len(), 1);
        let profile = select_profile();
        assert_eq!(profile.user_agent, obscura_net::BROWSER_USER_AGENT);
        assert_eq!(profile.platform, "Win32");
        assert_eq!(profile.ua_platform, "Windows");
        assert_eq!(profile.ua_platform_version, "15.0.0");
    }
}
