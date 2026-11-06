//! Storage policy types. Encryption and durable I/O are implemented in a
//! dedicated storage crate; this module defines the policy boundary.

/// Logical class used to apply different retention and encryption policies.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DataClass {
    /// Cookies, credentials, and site-specific state.
    SiteData,
    /// User-authored notes and documents.
    UserContent,
    /// Re-creatable cache entries.
    Cache,
    /// Browser session metadata.
    Session,
}

/// Whether data may be persisted across application restarts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Persistence {
    Persistent,
    SessionOnly,
    Ephemeral,
}
