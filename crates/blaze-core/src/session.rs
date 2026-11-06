//! Browser session data independent of any rendering engine.

use crate::TabId;

/// Metadata needed to restore a tab without retaining its live renderer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TabSnapshot {
    pub id: TabId,
    pub url: String,
    pub title: Option<String>,
    pub pinned: bool,
}

impl TabSnapshot {
    pub fn new(id: TabId, url: impl Into<String>) -> Self {
        Self {
            id,
            url: url.into(),
            title: None,
            pinned: false,
        }
    }
}
