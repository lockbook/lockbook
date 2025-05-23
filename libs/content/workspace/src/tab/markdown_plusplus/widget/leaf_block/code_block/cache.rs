use lb_rs::model::text::offset_types::DocCharOffset;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use syntect::highlighting::Style;

#[derive(Clone, Default)]
pub struct SyntaxHighlightCache {
    map: RefCell<HashMap<String, Vec<(Style, (DocCharOffset, DocCharOffset))>>>, // interior mutability
    used_this_frame: RefCell<HashSet<String>>,
}

impl SyntaxHighlightCache {
    pub fn insert(&self, key: String, value: Vec<(Style, (DocCharOffset, DocCharOffset))>) {
        self.used_this_frame.borrow_mut().insert(key.clone());
        self.map.borrow_mut().insert(key, value);
    }

    pub fn get(&self, key: &str) -> Option<Vec<(Style, (DocCharOffset, DocCharOffset))>> {
        self.used_this_frame.borrow_mut().insert(key.to_string());
        self.map.borrow().get(key).cloned()
    }

    pub fn garbage_collect(&self) {
        // remove unused entries
        let keys: Vec<String> = self.map.borrow().keys().cloned().collect();
        let used = self.used_this_frame.borrow();
        let mut map = self.map.borrow_mut();
        for key in keys {
            if !used.contains(&key) {
                map.remove(&key);
            }
        }
    }

    pub fn clear(&self) {
        self.map.borrow_mut().clear();
        self.used_this_frame.borrow_mut().clear();
    }
}
