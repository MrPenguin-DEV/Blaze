//! Memory-aware tab lifecycle policy. This crate tracks policy state only;
//! a browser engine must implement the actual renderer/process suspension.

use crate::memory::{MemoryBudget, TabActivity};
use crate::session::TabSnapshot;
use crate::TabId;

/// A tab plus the state needed by the scheduler.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TabEntry {
    pub snapshot: TabSnapshot,
    pub activity: TabActivity,
    pub last_focused_tick: u64,
    pub dirty: bool,
}

impl TabEntry {
    pub fn id(&self) -> TabId {
        self.snapshot.id
    }
}

/// Schedules eligible tabs for suspension when the live-tab ceiling is exceeded.
#[derive(Debug)]
pub struct TabManager {
    entries: Vec<TabEntry>,
    selected: Option<TabId>,
    next_id: u64,
    tick: u64,
    max_live_tabs: usize,
    budget: MemoryBudget,
}

impl TabManager {
    /// Creates a manager. At least one live tab is always allowed.
    pub fn new(budget: MemoryBudget, max_live_tabs: usize) -> Self {
        Self {
            entries: Vec::new(),
            selected: None,
            next_id: 1,
            tick: 0,
            max_live_tabs: max_live_tabs.max(1),
            budget,
        }
    }

    pub fn open(&mut self, url: impl Into<String>) -> TabId {
        let id = TabId::from_raw(self.next_id);
        self.next_id = self.next_id.saturating_add(1);
        self.tick = self.tick.saturating_add(1);

        for entry in &mut self.entries {
            if entry.activity == TabActivity::Foreground {
                entry.activity = TabActivity::Background;
            }
        }

        self.entries.push(TabEntry {
            snapshot: TabSnapshot::new(id, url),
            activity: TabActivity::Foreground,
            last_focused_tick: self.tick,
            dirty: false,
        });
        self.selected = Some(id);
        id
    }

    pub fn select(&mut self, id: TabId) -> bool {
        if !self.entries.iter().any(|entry| entry.id() == id) {
            return false;
        }

        self.tick = self.tick.saturating_add(1);
        for entry in &mut self.entries {
            if entry.id() == id {
                entry.activity = TabActivity::Foreground;
                entry.last_focused_tick = self.tick;
            } else if entry.activity == TabActivity::Foreground {
                entry.activity = TabActivity::Background;
            }
        }
        self.selected = Some(id);
        true
    }

    pub fn close(&mut self, id: TabId) -> bool {
        let old_len = self.entries.len();
        self.entries.retain(|entry| entry.id() != id);
        if self.entries.len() == old_len {
            return false;
        }

        if self.selected == Some(id) {
            self.selected = self.entries.last().map(TabEntry::id);
            if let Some(selected) = self.selected {
                let _ = self.select(selected);
            }
        }
        true
    }

    pub fn set_dirty(&mut self, id: TabId, dirty: bool) -> bool {
        if let Some(entry) = self.entries.iter_mut().find(|entry| entry.id() == id) {
            entry.dirty = dirty;
            true
        } else {
            false
        }
    }

    pub fn set_pinned(&mut self, id: TabId, pinned: bool) -> bool {
        if let Some(entry) = self.entries.iter_mut().find(|entry| entry.id() == id) {
            entry.snapshot.pinned = pinned;
            true
        } else {
            false
        }
    }

    /// Suspends least-recently-focused background tabs until the live-tab
    /// ceiling is met. The foreground, pinned, and dirty tabs are protected.
    /// Returns the IDs whose renderer resources should be released.
    pub fn enforce_live_limit(&mut self) -> Vec<TabId> {
        let mut live_count = self
            .entries
            .iter()
            .filter(|entry| entry.activity != TabActivity::Suspended)
            .count();

        if live_count <= self.max_live_tabs {
            return Vec::new();
        }

        let mut candidates: Vec<(u64, TabId)> = self
            .entries
            .iter()
            .filter(|entry| {
                entry.activity == TabActivity::Background
                    && !entry.snapshot.pinned
                    && !entry.dirty
            })
            .map(|entry| (entry.last_focused_tick, entry.id()))
            .collect();
        candidates.sort_by_key(|candidate| candidate.0);

        let mut suspended = Vec::new();
        for (_, id) in candidates {
            if live_count <= self.max_live_tabs {
                break;
            }
            if let Some(entry) = self.entries.iter_mut().find(|entry| entry.id() == id) {
                entry.activity = TabActivity::Suspended;
                suspended.push(id);
                live_count -= 1;
            }
        }
        suspended
    }

    pub fn selected(&self) -> Option<TabId> {
        self.selected
    }

    pub fn budget(&self) -> MemoryBudget {
        self.budget
    }

    pub fn entries(&self) -> &[TabEntry] {
        &self.entries
    }

    pub fn live_count(&self) -> usize {
        self.entries
            .iter()
            .filter(|entry| entry.activity != TabActivity::Suspended)
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suspends_oldest_background_tabs_but_keeps_foreground() {
        let mut tabs = TabManager::new(MemoryBudget::new(768), 1);
        let first = tabs.open("https://one.example");
        let second = tabs.open("https://two.example");
        let third = tabs.open("https://three.example");

        let suspended = tabs.enforce_live_limit();
        assert_eq!(suspended, vec![first, second]);
        assert_eq!(tabs.live_count(), 1);
        assert_eq!(tabs.selected(), Some(third));
    }

    #[test]
    fn pinned_and_dirty_tabs_are_not_suspended() {
        let mut tabs = TabManager::new(MemoryBudget::new(768), 1);
        let pinned = tabs.open("https://pinned.example");
        let dirty = tabs.open("https://dirty.example");
        let current = tabs.open("https://current.example");
        tabs.set_pinned(pinned, true);
        tabs.set_dirty(dirty, true);

        let suspended = tabs.enforce_live_limit();
        assert!(suspended.is_empty());
        assert_eq!(tabs.live_count(), 3);
        assert_eq!(tabs.selected(), Some(current));
    }

    #[test]
    fn selecting_suspended_tab_restores_it_to_foreground_policy() {
        let mut tabs = TabManager::new(MemoryBudget::new(512), 1);
        let first = tabs.open("https://first.example");
        let _second = tabs.open("https://second.example");
        tabs.enforce_live_limit();
        assert!(tabs.select(first));
        assert_eq!(
            tabs.entries().iter().find(|entry| entry.id() == first).unwrap().activity,
            TabActivity::Foreground
        );
    }
}
