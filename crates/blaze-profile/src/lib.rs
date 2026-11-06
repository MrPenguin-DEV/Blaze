//! Encrypted local browser profile and default-deny site permission policy.
//!
//! Permission decisions are profile data, not substitutes for browser-engine
//! enforcement. A connected engine must consult this policy before exposing
//! camera, microphone, geolocation, clipboard, notifications, or downloads.

use serde::{Deserialize, Serialize};
use std::path::Path;
use thiserror::Error;
use url::Url;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub enum PermissionKind {
    Camera,
    Microphone,
    Geolocation,
    Notifications,
    ClipboardRead,
    ClipboardWrite,
    Downloads,
    Popups,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum PermissionDecision {
    /// No saved decision. The engine should prompt; it must not auto-allow.
    #[default]
    Ask,
    Allow,
    Deny,
}

impl PermissionDecision {
    pub const fn is_allowed(self) -> bool {
        matches!(self, Self::Allow)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SitePermission {
    pub origin: String,
    pub kind: PermissionKind,
    pub decision: PermissionDecision,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Bookmark {
    pub title: String,
    pub url: String,
    pub folder: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub title: String,
    pub url: String,
    pub visited_unix_seconds: u64,
}

/// Minimal browser tab data persisted in the encrypted profile. It contains
/// no renderer memory or process state; tabs are reloaded from their URLs.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SessionTab {
    pub title: String,
    pub url: String,
    pub pinned: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BrowserSettings {
    pub homepage: String,
    pub do_not_track: bool,
    pub global_privacy_control: bool,
    pub block_third_party_cookies: bool,
    pub telemetry_enabled: bool,
    pub restore_last_session: bool,
}

impl Default for BrowserSettings {
    fn default() -> Self {
        Self {
            homepage: "about:blank".to_owned(),
            do_not_track: true,
            global_privacy_control: true,
            block_third_party_cookies: true,
            telemetry_enabled: false,
            restore_last_session: true,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct BrowserProfile {
    pub schema_version: u32,
    pub settings: BrowserSettings,
    pub bookmarks: Vec<Bookmark>,
    pub history: Vec<HistoryEntry>,
    pub permissions: Vec<SitePermission>,
    /// Backward-compatible field added in profile schema version 2.
    #[serde(default)]
    pub last_session: Vec<SessionTab>,
}

#[derive(Debug, Error)]
pub enum ProfileError {
    #[error("site URL is invalid")]
    InvalidSiteUrl,
    #[error("only HTTP and HTTPS origins can have saved site permissions")]
    UnsupportedOrigin,
    #[error("profile serialization failed: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("encrypted profile operation failed: {0}")]
    Vault(#[from] blaze_vault::VaultError),
    #[error("profile schema version {0} is newer than this Blaze build")]
    UnsupportedSchema(u32),
}

impl BrowserProfile {
    pub fn permission_for(
        &self,
        site_url: &str,
        kind: PermissionKind,
    ) -> Result<PermissionDecision, ProfileError> {
        let origin = normalize_origin(site_url)?;
        Ok(self
            .permissions
            .iter()
            .rev()
            .find(|entry| entry.origin == origin && entry.kind == kind)
            .map(|entry| entry.decision)
            .unwrap_or_default())
    }

    pub fn allows(&self, site_url: &str, kind: PermissionKind) -> Result<bool, ProfileError> {
        Ok(self.permission_for(site_url, kind)?.is_allowed())
    }

    pub fn set_permission(
        &mut self,
        site_url: &str,
        kind: PermissionKind,
        decision: PermissionDecision,
    ) -> Result<(), ProfileError> {
        let origin = normalize_origin(site_url)?;
        if let Some(existing) = self
            .permissions
            .iter_mut()
            .find(|entry| entry.origin == origin && entry.kind == kind)
        {
            existing.decision = decision;
        } else {
            self.permissions.push(SitePermission {
                origin,
                kind,
                decision,
            });
        }
        Ok(())
    }

    pub fn clear_site_permissions(&mut self, site_url: &str) -> Result<(), ProfileError> {
        let origin = normalize_origin(site_url)?;
        self.permissions.retain(|entry| entry.origin != origin);
        Ok(())
    }

    pub fn add_bookmark(&mut self, title: impl Into<String>, url: impl Into<String>) {
        let url = url.into();
        if self.bookmarks.iter().any(|bookmark| bookmark.url == url) {
            return;
        }
        self.bookmarks.push(Bookmark {
            title: title.into(),
            url,
            folder: None,
        });
    }

    pub fn remove_bookmark(&mut self, url: &str) -> bool {
        let old_len = self.bookmarks.len();
        self.bookmarks.retain(|bookmark| bookmark.url != url);
        old_len != self.bookmarks.len()
    }

    pub fn record_visit(
        &mut self,
        title: impl Into<String>,
        url: impl Into<String>,
        visited_unix_seconds: u64,
        max_entries: usize,
    ) {
        let url = url.into();
        self.history.retain(|entry| entry.url != url);
        self.history.push(HistoryEntry {
            title: title.into(),
            url,
            visited_unix_seconds,
        });
        self.history
            .sort_by_key(|entry| std::cmp::Reverse(entry.visited_unix_seconds));
        self.history.truncate(max_entries);
    }

    pub fn set_last_session(&mut self, tabs: Vec<SessionTab>) {
        self.last_session = tabs.into_iter().take(100).collect();
    }

    pub fn clear_history(&mut self) {
        self.history.clear();
    }

    pub fn save_encrypted(
        &self,
        path: impl AsRef<Path>,
        passphrase: &[u8],
    ) -> Result<(), ProfileError> {
        let mut profile = self.clone();
        profile.schema_version = 2;
        let serialized = serde_json::to_vec(&profile)?;
        blaze_vault::write_encrypted(path, passphrase, &serialized)?;
        Ok(())
    }

    pub fn load_encrypted(
        path: impl AsRef<Path>,
        passphrase: &[u8],
    ) -> Result<Self, ProfileError> {
        let serialized = blaze_vault::read_encrypted(path, passphrase)?;
        let profile: Self = serde_json::from_slice(&serialized)?;
        if profile.schema_version > 2 {
            return Err(ProfileError::UnsupportedSchema(profile.schema_version));
        }
        Ok(profile)
    }
}

fn normalize_origin(site_url: &str) -> Result<String, ProfileError> {
    let parsed = Url::parse(site_url).map_err(|_| ProfileError::InvalidSiteUrl)?;
    match parsed.scheme() {
        "http" | "https" => Ok(parsed.origin().ascii_serialization()),
        _ => Err(ProfileError::UnsupportedOrigin),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_FILE: AtomicU64 = AtomicU64::new(1);

    fn temp_file() -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "blaze-profile-{}-{}.blz",
            std::process::id(),
            NEXT_FILE.fetch_add(1, Ordering::Relaxed)
        ))
    }

    #[test]
    fn permissions_are_default_ask_and_origin_scoped() {
        let mut profile = BrowserProfile::default();
        assert_eq!(
            profile.permission_for("https://example.com/path", PermissionKind::Camera).unwrap(),
            PermissionDecision::Ask
        );
        assert!(!profile.allows("https://example.com", PermissionKind::Camera).unwrap());

        profile
            .set_permission("https://example.com/a", PermissionKind::Camera, PermissionDecision::Allow)
            .unwrap();
        assert!(profile.allows("https://example.com/b", PermissionKind::Camera).unwrap());
        assert!(!profile.allows("https://other.example", PermissionKind::Camera).unwrap());
    }

    #[test]
    fn insecure_or_non_web_origins_are_rejected() {
        let mut profile = BrowserProfile::default();
        assert!(matches!(
            profile.set_permission("file:///etc/passwd", PermissionKind::Camera, PermissionDecision::Allow),
            Err(ProfileError::UnsupportedOrigin)
        ));
    }

    #[test]
    fn session_metadata_is_capped_and_round_trips() {
        let mut profile = BrowserProfile::default();
        profile.set_last_session(vec![
            SessionTab { title: "First".into(), url: "https://first.example/".into(), pinned: true },
            SessionTab { title: "Second".into(), url: "https://second.example/".into(), pinned: false },
        ]);
        assert_eq!(profile.last_session.len(), 2);

        let path = temp_file();
        profile.save_encrypted(&path, b"session passphrase").unwrap();
        let loaded = BrowserProfile::load_encrypted(&path, b"session passphrase").unwrap();
        assert_eq!(loaded.last_session, profile.last_session);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn older_profile_without_session_field_is_still_readable() {
        let old = r#"{"schema_version":1,"settings":{"homepage":"about:blank","do_not_track":true,"global_privacy_control":true,"block_third_party_cookies":true,"telemetry_enabled":false,"restore_last_session":true},"bookmarks":[],"history":[],"permissions":[]}"#;
        let decoded: BrowserProfile = serde_json::from_str(old).unwrap();
        assert!(decoded.last_session.is_empty());
    }

    #[test]
    fn encrypted_profile_round_trip() {
        let path = temp_file();
        let mut profile = BrowserProfile::default();
        profile.add_bookmark("Example", "https://example.com/");
        profile
            .set_permission("https://example.com", PermissionKind::Geolocation, PermissionDecision::Deny)
            .unwrap();

        profile.save_encrypted(&path, b"strong passphrase").unwrap();
        let loaded = BrowserProfile::load_encrypted(&path, b"strong passphrase").unwrap();
        assert_eq!(loaded.bookmarks, profile.bookmarks);
        assert_eq!(loaded.permissions, profile.permissions);
        assert!(BrowserProfile::load_encrypted(&path, b"wrong passphrase").is_err());
        let _ = std::fs::remove_file(path);
    }
}
