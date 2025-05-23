use lb_rs::model::text::offset_types::{DocCharOffset, IntoRangeExt, RangeExt};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use syntect::highlighting::Style;

#[derive(Clone, Eq, PartialEq, Hash, Debug)]
struct SyntaxCacheKey {
    text: String,
    range: (DocCharOffset, DocCharOffset),
}

impl SyntaxCacheKey {
    fn new(text: String, range: (DocCharOffset, DocCharOffset)) -> Self {
        Self { text, range }
    }
}

pub type SyntaxHighlightResult = Vec<(Style, (DocCharOffset, DocCharOffset))>;

#[derive(Clone, Default)]
pub struct SyntaxHighlightCache {
    map: RefCell<HashMap<SyntaxCacheKey, SyntaxHighlightResult>>,
    used_this_frame: RefCell<HashSet<SyntaxCacheKey>>,
}

impl SyntaxHighlightCache {
    pub fn insert(
        &self,
        text: String,
        range: (DocCharOffset, DocCharOffset),
        value: SyntaxHighlightResult,
    ) {
        let key = SyntaxCacheKey::new(text, range);
        self.used_this_frame.borrow_mut().insert(key.clone());
        self.map.borrow_mut().insert(key, value);
    }

    pub fn get(
        &self,
        text: &str,
        range: (DocCharOffset, DocCharOffset),
    ) -> Option<SyntaxHighlightResult> {
        let key = SyntaxCacheKey::new(text.to_string(), range);
        self.used_this_frame.borrow_mut().insert(key.clone());
        self.map.borrow().get(&key).cloned()
    }

    pub fn garbage_collect(&self) {
        // Remove entries that weren't accessed this frame
        let keys: Vec<SyntaxCacheKey> = self.map.borrow().keys().cloned().collect();
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