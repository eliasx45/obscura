//! A bounded, read-oriented browser session for public tasks.
//!
//! Clianta remains the owner of task planning and capability authorization.
//! This module owns only Obscura context/page lifecycle, persistent storage,
//! request policy enforcement, DOM observation, and screenshots.

use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use obscura_net::interceptor::{InterceptAction, RequestInterceptor};
use obscura_net::{is_forbidden_ip, RequestInfo};
use thiserror::Error;
use url::Url;

use crate::{BrowserContext, Page, PageError};

const DEFAULT_NAVIGATION_TIMEOUT: Duration = Duration::from_secs(30);
const DEFAULT_VIEWPORT: (f32, f32) = (1280.0, 720.0);

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PublicTaskError {
    #[error("public task scope is required: provide a start URL or an allowed domain")]
    ScopeRequired,
    #[error("invalid public task URL: {0}")]
    InvalidUrl(String),
    #[error("public task URL is outside the allowed domains")]
    DomainNotAllowed,
    #[error("public task request was blocked by URL policy")]
    RequestBlocked,
    #[error("public task page has no matching content")]
    ContentNotFound,
    #[error("public task screenshot is unavailable in this build")]
    ScreenshotUnavailable,
    #[error("public task navigation failed: {0}")]
    Navigation(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PublicTaskPolicy {
    allowed_domains: Vec<String>,
    https_only: bool,
}

impl PublicTaskPolicy {
    /// Production public-site policy. Domains match exactly and their
    /// subdomains, so `example.com` permits `www.example.com` but not
    /// `example.com.evil.test`.
    pub fn new(
        allowed_domains: impl IntoIterator<Item = impl AsRef<str>>,
    ) -> Result<Self, PublicTaskError> {
        Self::with_https_only(allowed_domains, true)
    }

    /// The non-HTTPS option exists for deterministic local engine fixtures.
    /// Public production sessions should use [`Self::new`].
    pub fn with_https_only(
        allowed_domains: impl IntoIterator<Item = impl AsRef<str>>,
        https_only: bool,
    ) -> Result<Self, PublicTaskError> {
        let mut normalized = Vec::new();
        for domain in allowed_domains {
            let domain = normalize_domain(domain.as_ref())?;
            if domain.parse::<IpAddr>().is_ok() {
                return Err(PublicTaskError::InvalidUrl(
                    "allowed domains must be hostnames".into(),
                ));
            }
            if !normalized.iter().any(|existing| existing == &domain) {
                normalized.push(domain);
            }
        }
        if normalized.is_empty() {
            return Err(PublicTaskError::ScopeRequired);
        }
        Ok(Self {
            allowed_domains: normalized,
            https_only,
        })
    }

    /// Derive a host scope from the start URL when no explicit domains were
    /// supplied. Never default to the open internet.
    pub fn from_scope(
        start_url: Option<&str>,
        allowed_domains: &[String],
    ) -> Result<Self, PublicTaskError> {
        if !allowed_domains.is_empty() {
            return Self::new(allowed_domains.iter().map(String::as_str));
        }
        let start_url = start_url.ok_or(PublicTaskError::ScopeRequired)?;
        let url = parse_url_shape(start_url, true)?;
        let host = url
            .host_str()
            .ok_or_else(|| PublicTaskError::InvalidUrl("URL host is required".into()))?;
        Self::new([host]).and_then(|policy| {
            policy.validate(start_url).map(|_| policy)
        })
    }

    pub fn allowed_domains(&self) -> &[String] {
        &self.allowed_domains
    }

    pub fn validate(&self, raw_url: &str) -> Result<Url, PublicTaskError> {
        let url = parse_url_shape(raw_url, self.https_only)?;
        let host = url
            .host_str()
            .ok_or_else(|| PublicTaskError::InvalidUrl("URL host is required".into()))?
            .trim_end_matches('.')
            .to_ascii_lowercase();
        if !self.allowed_domains.iter().any(|domain| {
            host == *domain
                || host
                    .strip_suffix(domain)
                    .is_some_and(|prefix| prefix.ends_with('.'))
        }) {
            return Err(PublicTaskError::DomainNotAllowed);
        }
        Ok(url)
    }
}

fn normalize_domain(raw: &str) -> Result<String, PublicTaskError> {
    let domain = raw.trim().trim_end_matches('.').to_ascii_lowercase();
    if domain.is_empty()
        || domain.contains('/')
        || domain.contains(':')
        || domain.contains('@')
        || domain.chars().any(char::is_whitespace)
    {
        return Err(PublicTaskError::InvalidUrl(
            "allowed domain must be a hostname".into(),
        ));
    }
    Ok(domain)
}

fn parse_url_shape(raw_url: &str, https_only: bool) -> Result<Url, PublicTaskError> {
    let url = Url::parse(raw_url).map_err(|error| PublicTaskError::InvalidUrl(error.to_string()))?;
    if https_only && url.scheme() != "https" {
        return Err(PublicTaskError::InvalidUrl(
            "public tasks require https URLs".into(),
        ));
    }
    if !matches!(url.scheme(), "http" | "https") {
        return Err(PublicTaskError::InvalidUrl(
            "only http and https URLs are supported".into(),
        ));
    }
    if url.username() != "" || url.password().is_some() {
        return Err(PublicTaskError::InvalidUrl(
            "URL credentials are not allowed".into(),
        ));
    }
    if url.port().is_some_and(|port| port != 443) {
        return Err(PublicTaskError::InvalidUrl(
            "unsafe URL ports are not allowed".into(),
        ));
    }
    let host = url
        .host_str()
        .ok_or_else(|| PublicTaskError::InvalidUrl("URL host is required".into()))?;
    let lower_host = host.trim_end_matches('.').to_ascii_lowercase();
    if lower_host == "localhost"
        || lower_host.ends_with(".localhost")
        || lower_host == "metadata.google.internal"
        || lower_host == "metadata"
    {
        return Err(PublicTaskError::InvalidUrl(
            "private and metadata hosts are not allowed".into(),
        ));
    }
    if let Ok(ip) = host.parse::<IpAddr>() {
        if is_forbidden_ip(ip) {
            return Err(PublicTaskError::InvalidUrl(
                "private and metadata IPs are not allowed".into(),
            ));
        }
    }
    Ok(url)
}

#[derive(Clone)]
struct PublicTaskRequestInterceptor {
    policy: PublicTaskPolicy,
}

#[async_trait]
impl RequestInterceptor for PublicTaskRequestInterceptor {
    async fn intercept(&self, request: &RequestInfo) -> InterceptAction {
        match self.policy.validate(request.url.as_str()) {
            Ok(_) => InterceptAction::Continue,
            Err(_) => InterceptAction::Block,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicTaskObservation {
    pub url: String,
    pub title: String,
    pub text: String,
}

/// One isolated Obscura-backed public-task session.
pub struct PublicTaskSession {
    context: Arc<BrowserContext>,
    page: Page,
    policy: PublicTaskPolicy,
}

impl PublicTaskSession {
    pub fn new(
        id: impl Into<String>,
        storage_dir: impl Into<std::path::PathBuf>,
        policy: PublicTaskPolicy,
    ) -> Self {
        let context = Arc::new(BrowserContext::with_storage(
            id.into(),
            Some(storage_dir.into()),
        ));
        let interceptor = PublicTaskRequestInterceptor {
            policy: policy.clone(),
        };
        if let Ok(mut guard) = context.http_client.interceptor.try_write() {
            *guard = Some(Box::new(interceptor));
        }
        let mut page = Page::new("public-task-page".into(), context.clone());
        page.set_navigation_timeout(DEFAULT_NAVIGATION_TIMEOUT);
        page.navigate_blank();
        Self {
            context,
            page,
            policy,
        }
    }

    pub async fn open(
        id: impl Into<String>,
        storage_dir: impl Into<std::path::PathBuf>,
        start_url: Option<&str>,
        allowed_domains: &[String],
    ) -> Result<Self, PublicTaskError> {
        let policy = PublicTaskPolicy::from_scope(start_url, allowed_domains)?;
        let mut session = Self::new(id, storage_dir, policy);
        if let Some(start_url) = start_url {
            session.navigate(start_url).await?;
        }
        Ok(session)
    }

    pub fn policy(&self) -> &PublicTaskPolicy {
        &self.policy
    }

    pub async fn navigate(&mut self, raw_url: &str) -> Result<(), PublicTaskError> {
        let url = self.policy.validate(raw_url)?;
        self.page
            .navigate_with_wait(url.as_str(), crate::WaitUntil::Load)
            .await
            .map_err(|error| match error {
                PageError::NetworkError(message) if message.contains("blocked") => {
                    PublicTaskError::RequestBlocked
                }
                other => PublicTaskError::Navigation(other.to_string()),
            })
    }

    pub fn observe(&mut self) -> PublicTaskObservation {
        let title = self
            .page
            .evaluate("document.title")
            .as_str()
            .unwrap_or_default()
            .to_string();
        let text = self
            .page
            .evaluate("document.body ? document.body.innerText : ''")
            .as_str()
            .unwrap_or_default()
            .to_string();
        PublicTaskObservation {
            url: self.page.url_string(),
            title,
            text,
        }
    }

    /// Read text from a CSS selector. The selector is data, not JavaScript:
    /// it is escaped before being passed to the page's internal DOM helper.
    pub fn extract_text(&mut self, selector: &str) -> Result<String, PublicTaskError> {
        let selector = selector.replace('\\', "\\\\").replace('\'', "\\'");
        let value = self.page.evaluate(&format!(
            "(function() {{ var el = document.querySelector('{}'); return el ? el.textContent : null; }})()",
            selector
        ));
        value
            .as_str()
            .map(str::to_string)
            .ok_or(PublicTaskError::ContentNotFound)
    }

    #[cfg(feature = "render")]
    pub async fn screenshot(&mut self) -> Result<Vec<u8>, PublicTaskError> {
        self.page.prepare_screenshot_resources(5_000).await;
        self.page
            .screenshot(DEFAULT_VIEWPORT)
            .ok_or(PublicTaskError::ScreenshotUnavailable)
    }

    pub fn close(self) {
        drop(self);
    }
}

impl Drop for PublicTaskSession {
    fn drop(&mut self) {
        self.context.save_cookies();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derives_scope_from_start_url() {
        let policy = PublicTaskPolicy::from_scope(Some("https://example.com"), &[]).unwrap();
        assert_eq!(policy.allowed_domains(), &["example.com"]);
        assert!(policy.validate("https://www.example.com/path").is_ok());
        assert_eq!(
            policy.validate("https://example.com.evil.test").unwrap_err(),
            PublicTaskError::DomainNotAllowed
        );
    }

    #[test]
    fn rejects_unsafe_public_urls() {
        let policy = PublicTaskPolicy::new(["example.com"]).unwrap();
        for raw in [
            "http://example.com",
            "https://user:pass@example.com",
            "https://example.com:8443",
            "https://localhost",
            "https://127.0.0.1",
            "https://metadata.google.internal",
            "https://other.example.test",
        ] {
            assert!(policy.validate(raw).is_err(), "{raw} must be rejected");
        }
    }

    #[test]
    fn requires_scope_instead_of_open_internet() {
        assert_eq!(
            PublicTaskPolicy::from_scope(None, &[]).unwrap_err(),
            PublicTaskError::ScopeRequired
        );
    }

    #[test]
    fn session_starts_on_a_clean_blank_page() {
        let dir = std::env::temp_dir().join(format!(
            "obscura-public-task-test-{}",
            std::process::id()
        ));
        let mut session = PublicTaskSession::new(
            "test-public-task",
            &dir,
            PublicTaskPolicy::new(["example.com"]).unwrap(),
        );
        let observation = session.observe();
        assert_eq!(observation.url, "about:blank");
        assert!(observation.text.is_empty());
    }
}
