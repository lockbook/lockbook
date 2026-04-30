use std::collections::HashMap;

use crate::tab::{Destination, Tab};

/// Double-buffered tab cache. Tabs accessed during a frame (via promote)
/// survive; unaccessed tabs are evicted at end_frame unless dirty.
pub struct TabCache {
    current: HashMap<Destination, Tab>,
    previous: HashMap<Destination, Tab>,
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
        for (dest, tab) in self.previous.drain() {
            if tab.last_changed > tab.last_saved {
                keep.push((dest, tab));
            }
        }
        for (dest, tab) in keep {
            self.current.insert(dest, tab);
        }
    }

    /// Move a tab from previous into current, keeping it alive this frame.
    pub fn promote(&mut self, dest: &Destination) {
        if !self.current.contains_key(dest) {
            if let Some(tab) = self.previous.remove(dest) {
                self.current.insert(dest.clone(), tab);
            }
        }
    }

    pub fn get(&self, dest: &Destination) -> Option<&Tab> {
        self.current.get(dest)
    }

    pub fn get_mut(&mut self, dest: &Destination) -> Option<&mut Tab> {
        self.current.get_mut(dest)
    }

    /// Search both current and previous. Used by check_launch and save
    /// completion to find tabs that are dirty but not promoted this frame.
    pub fn get_any(&self, dest: &Destination) -> Option<&Tab> {
        self.current.get(dest).or_else(|| self.previous.get(dest))
    }

    pub fn get_any_mut(&mut self, dest: &Destination) -> Option<&mut Tab> {
        if self.current.contains_key(dest) {
            self.current.get_mut(dest)
        } else {
            self.previous.get_mut(dest)
        }
    }

    pub fn insert(&mut self, dest: Destination, tab: Tab) -> Option<Tab> {
        self.current.insert(dest, tab)
    }

    pub fn remove(&mut self, dest: &Destination) -> Option<Tab> {
        self.current
            .remove(dest)
            .or_else(|| self.previous.remove(dest))
    }

    pub fn contains_key(&self, dest: &Destination) -> bool {
        self.current.contains_key(dest) || self.previous.contains_key(dest)
    }

    pub fn values(&self) -> impl Iterator<Item = &Tab> {
        self.current.values().chain(self.previous.values())
    }

    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut Tab> {
        self.current.values_mut().chain(self.previous.values_mut())
    }

    pub fn iter(&self) -> impl Iterator<Item = (&Destination, &Tab)> {
        self.current.iter().chain(self.previous.iter())
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&Destination, &mut Tab)> {
        self.current.iter_mut().chain(self.previous.iter_mut())
    }

    pub fn keys(&self) -> Vec<Destination> {
        self.current
            .keys()
            .chain(self.previous.keys())
            .cloned()
            .collect()
    }

    pub fn retain(&mut self, f: impl FnMut(&Destination, &mut Tab) -> bool) {
        self.current.retain(f);
    }
}
