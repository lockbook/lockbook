use comrak::nodes::AstNode;
use lb_rs::model::text::offset_types::{DocCharOffset, RangeExt as _, RangeIterExt as _};

use crate::tab::markdown_plusplus::MarkdownPlusPlus;

pub(crate) mod code_block;
pub(crate) mod heading;
pub(crate) mod html_block;
pub(crate) mod paragraph;
pub(crate) mod table_cell;
pub(crate) mod thematic_break;

impl<'ast> MarkdownPlusPlus {
    /// Returns 5 ranges representing the pre-node line, pre-first-child section,
    /// inter-children section, post-last-child section, and post-node line.
    /// Returns None if there are no children on this line.
    #[allow(clippy::type_complexity)]
    fn line_ranges(
        &self, node: &'ast AstNode<'ast>, node_line: (DocCharOffset, DocCharOffset),
    ) -> Option<(
        (DocCharOffset, DocCharOffset),
        (DocCharOffset, DocCharOffset),
        (DocCharOffset, DocCharOffset),
        (DocCharOffset, DocCharOffset),
        (DocCharOffset, DocCharOffset),
    )> {
        let children = self.children_in_range(node, node_line);
        if children.is_empty() {
            return None;
        }

        let node_range = self.node_range(node);
        let node_range =
            (node_range.start().max(node_line.start()), node_range.end().min(node_line.end()));

        let first_child = children.first().unwrap();
        let first_child_range = self.node_range(first_child);
        let last_child = children.last().unwrap();
        let last_child_range = self.node_range(last_child);

        let pre_node_line = (node_line.start(), node_range.start());
        let pre_first_child = (node_range.start(), first_child_range.start());
        let inter_children = (first_child_range.start(), last_child_range.end());
        let post_last_child = (last_child_range.end(), node_range.end());
        let post_node_line = (node_range.end(), node_line.end());

        Some((pre_node_line, pre_first_child, inter_children, post_last_child, post_node_line))
    }

    /// Returns true if the node's lines intersect the selection. Differs from
    /// node_lines_intersect_selection in cases where the selection intersects
    /// optional indentation, trailing whitespace, or the portion of a node's
    /// lines that are due to container blocks.
    pub fn node_lines_intersect_selection(&self, node: &'ast AstNode<'ast>) -> bool {
        for line_idx in self.node_lines(node).iter() {
            let line = self.bounds.source_lines[line_idx];
            let node_line = self.node_line(node, line);
            if node_line.intersects(&self.buffer.current.selection, true) {
                return true;
            }
        }
        false
    }

    pub fn children_in_range(
        &self, node: &'ast AstNode<'ast>, range: (DocCharOffset, DocCharOffset),
    ) -> Vec<&'ast AstNode<'ast>> {
        let mut children = Vec::new();
        for child in self.sorted_children(node) {
            if range.contains_range(&self.node_range(child), true, true) {
                children.push(child);
            }
        }
        children
    }
}
