//! Browser navigation policy and the boundary for a real rendering engine.
//!
//! No renderer is provided by this crate yet. A Servo adapter must implement
//! the contract and connect its event loop and RenderingContext to the shell.

use blaze_core::TabId;
use thiserror::Error;
use url::Url;

#[derive(Debug, Error, Clone, Eq, PartialEq)]
pub enum NavigationError {
    #[error("the address is empty")]
    Empty,
    #[error("the address is not a valid URL")]
    InvalidUrl,
    #[error("only http, https, and about:blank are supported")]
    UnsupportedScheme,
}

/// Parses a URL or converts a plain search phrase into a Google search query.
/// Dangerous or unsupported schemes are rejected before they reach an engine.
pub fn normalize_address(input: &str) -> Result<Url, NavigationError> {
    let input = input.trim();
    if input.is_empty() {
        return Err(NavigationError::Empty);
    }

    if input.eq_ignore_ascii_case("about:blank") {
        return Url::parse("about:blank").map_err(|_| NavigationError::InvalidUrl);
    }

    let lowered = input.to_ascii_lowercase();
    for scheme in ["javascript:", "data:", "file:", "ftp:", "chrome:", "blob:"] {
        if lowered.starts_with(scheme) {
            return Err(NavigationError::UnsupportedScheme);
        }
    }

    if input.chars().any(char::is_whitespace) && !input.contains("://") {
        let mut search = Url::parse("https://www.google.com/search?q=")
            .map_err(|_| NavigationError::InvalidUrl)?;
        search.query_pairs_mut().append_pair("q", input);
        return Ok(search);
    }

    let candidate = if input.contains("://") || input.starts_with("localhost:") {
        if input.starts_with("localhost:") {
            format!("http://{input}")
        } else {
            input.to_owned()
        }
    } else {
        format!("https://{input}")
    };
    let parsed = Url::parse(&candidate).map_err(|_| NavigationError::InvalidUrl)?;

    match parsed.scheme() {
        "http" | "https" => Ok(parsed),
        _ => Err(NavigationError::UnsupportedScheme),
    }
}

/// Declares features that a concrete engine adapter really supports.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct EngineCapabilities {
    pub page_rendering: bool,
    pub downloads: bool,
    pub hardware_video_decode: bool,
    pub gpu_compositing: bool,
    pub site_permissions: bool,
    pub accessibility: bool,
}

/// Current engine state surfaced to the shell.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EngineStatus {
    NotConnected,
    Initializing,
    Ready,
    Failed,
}

/// Narrow integration contract for Servo or another standards-based engine.
pub trait BrowserEngine {
    type Error;

    fn status(&self) -> EngineStatus;
    fn capabilities(&self) -> EngineCapabilities;
    fn navigate(&mut self, tab: TabId, url: &Url) -> Result<(), Self::Error>;
    fn close_tab(&mut self, tab: TabId) -> Result<(), Self::Error>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_http_and_https() {
        assert_eq!(
            normalize_address("example.com").unwrap().as_str(),
            "https://example.com/"
        );
        assert_eq!(
            normalize_address("http://localhost:8080").unwrap().scheme(),
            "http"
        );
        assert_eq!(
            normalize_address("localhost:8080").unwrap().scheme(),
            "http"
        );
    }

    #[test]
    fn rejects_script_and_file_schemes() {
        assert_eq!(
            normalize_address("javascript:alert(1)"),
            Err(NavigationError::UnsupportedScheme)
        );
        assert_eq!(
            normalize_address("file:///etc/passwd"),
            Err(NavigationError::UnsupportedScheme)
        );
    }

    #[test]
    fn converts_search_terms_to_search_url() {
        let url = normalize_address("blaze rust browser").unwrap();
        assert_eq!(url.host_str(), Some("www.google.com"));
        assert!(url.query().unwrap().contains("blaze"));
    }
}
