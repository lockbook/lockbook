use comrak::nodes::AstNode;
use lb_rs::model::text::offset_types::{DocCharOffset, RangeExt as _, RangeIterExt as _};

use crate::tab::markdown_editor::Editor;

pub(crate) mod code_block;
pub(crate) mod heading;
pub(crate) mod html_block;
pub(crate) mod paragraph;
pub(crate) mod table_cell;
pub(crate) mod thematic_break;

impl<'ast> Editor {
    /// Returns 5 ranges representing the pre-node range, pre-first-child section,
    /// inter-children section, post-last-child section, and post-node range.
    #[allow(clippy::type_complexity)]
    fn split_range(
        &self, node: &'ast AstNode<'ast>, range: (DocCharOffset, DocCharOffset),
    ) -> Option<(
        (DocCharOffset, DocCharOffset),
        (DocCharOffset, DocCharOffset),
        (DocCharOffset, DocCharOffset),
        (DocCharOffset, DocCharOffset),
        (DocCharOffset, DocCharOffset),
    )> {
        let node_range = self.node_range(node);

        let first_child_start = node
            .descendants()
            .skip(1)
            .filter(|descendant| self.node_range(descendant).intersects(&range, false))
            .map(|descendant| self.node_range(descendant).start())
            .min()?;
        let last_child_end = node
            .descendants()
            .skip(1)
            .filter(|descendant| self.node_range(descendant).intersects(&range, false))
            .map(|descendant| self.node_range(descendant).end())
            .max()?;

        let pre_node = (range.start(), node_range.start());
        let pre_first_child = (node_range.start(), first_child_start);
        let inter_children = (first_child_start, last_child_end);
        let post_last_child = (last_child_end, node_range.end());
        let post_node = (node_range.end(), range.end());

        Some((
            pre_node.trim(&range),
            pre_first_child.trim(&range),
            inter_children.trim(&range),
            post_last_child.trim(&range),
            post_node.trim(&range),
        ))
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
}
