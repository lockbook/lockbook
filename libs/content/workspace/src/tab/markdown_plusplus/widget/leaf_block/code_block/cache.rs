use std::collections::{HashMap, HashSet};
use syntect::highlighting::Style;

#[derive(Clone, Default)]
pub struct SyntaxHighlightCache {
    pub map: HashMap<String, Vec<(Style, String)>>,
    pub used_this_frame: HashSet<String>,
}

impl SyntaxHighlightCache {
    pub fn insert(&mut self, key: String, value: Vec<(Style, String)>) {
        self.used_this_frame.insert(key.clone());
        self.map.insert(key, value);
    }

    pub fn get(&mut self, key: &str) -> Option<&Vec<(Style, String)>> {
        self.used_this_frame.insert(key.to_string());
        self.map.get(key)
    }

    pub fn garbage_collect(&mut self) {
        // remove unused entries
        let keys: Vec<String> = self.map.keys().cloned().collect();
        for key in keys {
            if !self.used_this_frame.contains(&key) {
                self.map.remove(&key);
            }
        }
    }
}
