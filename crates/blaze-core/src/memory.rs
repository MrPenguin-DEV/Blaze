//! Resource policy primitives. These are policy inputs, not OS enforcement.

/// Lifecycle state used by the tab scheduler.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TabActivity {
    /// Currently visible and receiving user interaction.
    Foreground,
    /// Open but not visible; eligible for throttling.
    Background,
    /// Renderer resources may be released and restored from a snapshot.
    Suspended,
}

/// Configurable soft budget for browser-managed memory, in mebibytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MemoryBudget {
    soft_limit_mib: u32,
}

impl MemoryBudget {
    pub const fn new(soft_limit_mib: u32) -> Self {
        Self { soft_limit_mib }
    }

    pub const fn soft_limit_mib(self) -> u32 {
        self.soft_limit_mib
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn budget_keeps_configured_limit() {
        let budget = MemoryBudget::new(768);
        assert_eq!(budget.soft_limit_mib(), 768);
    }
}
