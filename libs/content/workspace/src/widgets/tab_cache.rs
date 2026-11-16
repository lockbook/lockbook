use std::collections::HashMap;

use lb_rs::Uuid;

use crate::tab::{SessionId, Tab};
use lb_rs::io::network::Network;
use lb_rs::io::server::Server;

/// Double-buffered tab cache, keyed by session id (not destination). Tabs
/// accessed during a frame (via promote) survive; unaccessed tabs are
/// evicted at end_frame unless dirty.
pub struct TabCache<S = Network> {
    current: HashMap<SessionId, Tab<S>>,
    previous: HashMap<SessionId, Tab<S>>,
}

impl<S: Server> Default for TabCache<S> {
    fn default() -> Self {
        Self::new()
    }
}

impl<S: Server> TabCache<S> {
    pub fn new() -> Self {
        Self { current: HashMap::new(), previous: HashMap::new() }
    }

    pub fn begin_frame(&mut self) {
        self.previous = std::mem::take(&mut self.current);
    }

    /// Dirty tabs are kept alive, closed ones included, so saves reach them.
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

    pub fn get(&self, id: &SessionId) -> Option<&Tab<S>> {
        self.current.get(id)
    }

    pub fn get_mut(&mut self, id: &SessionId) -> Option<&mut Tab<S>> {
        self.current.get_mut(id)
    }

    /// Search both current and previous. Used by check_launch and save
    /// completion to find tabs that are dirty but not promoted this frame.
    pub fn get_any(&self, id: &SessionId) -> Option<&Tab<S>> {
        self.current.get(id).or_else(|| self.previous.get(id))
    }

    pub fn get_any_mut(&mut self, id: &SessionId) -> Option<&mut Tab<S>> {
        if self.current.contains_key(id) {
            self.current.get_mut(id)
        } else {
            self.previous.get_mut(id)
        }
    }

    pub fn insert(&mut self, id: SessionId, tab: Tab<S>) -> Option<Tab<S>> {
        self.current.insert(id, tab)
    }

    pub fn remove(&mut self, id: &SessionId) -> Option<Tab<S>> {
        self.current.remove(id).or_else(|| self.previous.remove(id))
    }

    pub fn contains_key(&self, id: &SessionId) -> bool {
        self.current.contains_key(id) || self.previous.contains_key(id)
    }

    pub fn values(&self) -> impl Iterator<Item = &Tab<S>> {
        self.current.values().chain(self.previous.values())
    }

    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut Tab<S>> {
        self.current.values_mut().chain(self.previous.values_mut())
    }

    pub fn iter(&self) -> impl Iterator<Item = (&SessionId, &Tab<S>)> {
        self.current.iter().chain(self.previous.iter())
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&SessionId, &mut Tab<S>)> {
        self.current.iter_mut().chain(self.previous.iter_mut())
    }

    pub fn keys(&self) -> Vec<SessionId> {
        self.current
            .keys()
            .chain(self.previous.keys())
            .copied()
            .collect()
    }

    pub fn retain(&mut self, f: impl FnMut(&SessionId, &mut Tab<S>) -> bool) {
        self.current.retain(f);
    }

    /// Prefer `target` when set. Otherwise prefer a tab still waiting on this
    /// file so a second tab of the same dest receives its load instead of
    /// replacing an already-open sibling.
    pub fn find_for_load_mut(
        &mut self, id: Uuid, target: Option<SessionId>,
    ) -> Option<&mut Tab<S>> {
        if let Some(sid) = target {
            // The session may have moved on (e.g. replaced) since the load
            // was queued; its content must not land under another dest.
            return self
                .get_any_mut(&sid)
                .filter(|t| t.destination.backing_file() == Some(id));
        }
        let is_loading_file = |t: &Tab<S>| {
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
