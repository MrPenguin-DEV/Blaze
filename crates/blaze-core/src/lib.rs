//! Shared, platform-independent primitives for Blaze.
//!
//! This crate deliberately does not implement a browser engine. Engine-specific
//! integration belongs in a separate adapter so the shell and resource policy
//! can be tested independently.

pub mod memory;
pub mod session;
pub mod storage;
pub mod tabs;

/// Stable identifier for a tab within a browser session.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TabId(u64);

impl TabId {
    /// Creates an identifier from a value allocated by the session manager.
    pub const fn from_raw(value: u64) -> Self {
        Self(value)
    }

    /// Returns the underlying identifier.
    pub const fn get(self) -> u64 {
        self.0
    }
}
