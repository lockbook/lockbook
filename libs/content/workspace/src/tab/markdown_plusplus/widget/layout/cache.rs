use std::cell::RefCell;

use comrak::nodes::AstNode;
use lb_rs::model::text::offset_types::DocCharOffset;

use crate::tab::markdown_plusplus::MarkdownPlusPlus;

#[derive(Default)]
pub struct CacheEntry<T> {
    range: (DocCharOffset, DocCharOffset),
    value: T,
}

#[derive(Default)]
pub struct LayoutCache {
    pub height: RefCell<Vec<CacheEntry<f32>>>,
}

impl LayoutCache {
    pub fn clear(&self) {
        self.height.borrow_mut().clear();
    }
}

impl<'ast> MarkdownPlusPlus {
    pub fn get_cached_node_height(&self, node: &'ast AstNode<'ast>) -> Option<f32> {
        let range = self.node_range(node);
        self.layout_cache
            .height
            .borrow()
            .binary_search_by(|entry| entry.range.cmp(&range))
            .ok()
            .map(|i| self.layout_cache.height.borrow()[i].value)
    }

    pub fn set_cached_node_height(&self, node: &'ast AstNode<'ast>, height: f32) {
        let range = self.node_range(node);
        let mut cache = self.layout_cache.height.borrow_mut();
        match cache.binary_search_by(|entry| entry.range.cmp(&range)) {
            Ok(i) => cache[i].value = height,
            Err(i) => cache.insert(i, CacheEntry { range, value: height }),
        }
    }
}
