use std::collections::HashMap;

use lb_rs::Uuid;

use crate::tab::{SessionId, Tab};

/// Double-buffered tab cache, keyed by session id (not destination). Tabs
/// accessed during a frame (via promote) survive; unaccessed tabs are
/// evicted at end_frame unless dirty.
pub struct TabCache {
    current: HashMap<SessionId, Tab>,
    previous: HashMap<SessionId, Tab>,
}

impl Default for TabCache {
    fn default() -> Self {
        Self::new()
    }
}

impl TabCache {
    pub fn new() -> Self {
        Self { current: HashMap::new(), previous: HashMap::new() }
    }

    pub fn begin_frame(&mut self) {
        self.previous = std::mem::take(&mut self.current);
    }

    /// Dirty tabs are kept alive — their save was already queued at
    /// close time and needs the tab present for check_launch.
    pub fn end_frame(&mut self) {
        let mut keep = Vec::new();
        for (id, tab) in self.previous.drain() {
            if tab.last_changed > tab.last_saved {
                keep.push((id, tab));
            }
        }
        for (id, tab) in keep {
            self.current.insert(id, tab);
        }
    }

    /// Move a tab from previous into current, keeping it alive this frame.
    pub fn promote(&mut self, id: &SessionId) {
        if !self.current.contains_key(id) {
            if let Some(tab) = self.previous.remove(id) {
                self.current.insert(*id, tab);
            }
        }
    }

    pub fn get(&self, id: &SessionId) -> Option<&Tab> {
        self.current.get(id)
    }

    pub fn get_mut(&mut self, id: &SessionId) -> Option<&mut Tab> {
        self.current.get_mut(id)
    }

    /// Search both current and previous. Used by check_launch and save
    /// completion to find tabs that are dirty but not promoted this frame.
    pub fn get_any(&self, id: &SessionId) -> Option<&Tab> {
        self.current.get(id).or_else(|| self.previous.get(id))
    }

    pub fn get_any_mut(&mut self, id: &SessionId) -> Option<&mut Tab> {
        if self.current.contains_key(id) {
            self.current.get_mut(id)
        } else {
            self.previous.get_mut(id)
        }
    }

    pub fn insert(&mut self, id: SessionId, tab: Tab) -> Option<Tab> {
        self.current.insert(id, tab)
    }

    pub fn remove(&mut self, id: &SessionId) -> Option<Tab> {
        self.current.remove(id).or_else(|| self.previous.remove(id))
    }

    pub fn contains_key(&self, id: &SessionId) -> bool {
        self.current.contains_key(id) || self.previous.contains_key(id)
    }

    pub fn values(&self) -> impl Iterator<Item = &Tab> {
        self.current.values().chain(self.previous.values())
    }

    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut Tab> {
        self.current.values_mut().chain(self.previous.values_mut())
    }

    pub fn iter(&self) -> impl Iterator<Item = (&SessionId, &Tab)> {
        self.current.iter().chain(self.previous.iter())
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&SessionId, &mut Tab)> {
        self.current.iter_mut().chain(self.previous.iter_mut())
    }

    pub fn keys(&self) -> Vec<SessionId> {
        self.current
            .keys()
            .chain(self.previous.keys())
            .copied()
            .collect()
    }

    pub fn retain(&mut self, f: impl FnMut(&SessionId, &mut Tab) -> bool) {
        self.current.retain(f);
    }

    /// Prefer `target` when set. Otherwise prefer a tab still waiting on this
    /// file so a second tab of the same dest receives its load instead of
    /// replacing an already-open sibling.
    pub fn find_for_load_mut(&mut self, id: Uuid, target: Option<SessionId>) -> Option<&mut Tab> {
        if let Some(sid) = target {
            return self.get_any_mut(&sid);
        }
        let is_loading_file = |t: &Tab| {
            t.destination.backing_file() == Some(id)
                && matches!(t.content, crate::tab::ContentState::Loading(_))
        };
        let key = self
            .current
            .iter()
            .find(|(_, t)| is_loading_file(t))
            .map(|(k, _)| *k)
            .or_else(|| {
                self.previous
                    .iter()
                    .find(|(_, t)| is_loading_file(t))
                    .map(|(k, _)| *k)
            })
            .or_else(|| {
                self.iter()
                    .find(|(_, t)| t.destination.backing_file() == Some(id))
                    .map(|(k, _)| *k)
            })?;
        self.get_any_mut(&key)
    }
}
