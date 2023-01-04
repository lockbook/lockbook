use crate::cursor_types::DocByteOffset;
use crate::editor::Editor;
use crate::element::Element;
use crate::element::Element::Item;
use crate::theme::VisualAppearance;
use egui::TextFormat;
use std::cmp::{max, min};
use std::ops::Range;

#[derive(Debug, Clone)]
pub struct StyledChunk {
    pub block_start: bool,
    pub range: Range<DocByteOffset>,
    pub elements: Vec<Element>, // maybe a good tiny_vec candidate
}

impl StyledChunk {
    pub fn item_count(&self) -> usize {
        self.elements
            .iter()
            .filter(|el| matches!(el, Element::Item))
            .count()
    }

    pub fn text_format(&self, visual: &VisualAppearance) -> TextFormat {
        let mut text_format = TextFormat::default();
        for element in &self.elements {
            element.apply_style(&mut text_format, visual);
        }

        text_format
    }
}

impl Editor {
    /// Traverse an AST and determine 2 key things:
    /// * The elements that a region of text is impacted by
    /// * Which regions of text should be in their own galley
    ///
    /// This is a recursive function that processes a given node, and it's children. It has to
    /// capture text that may occur before the first child, between children, and after the last
    /// child. The rest is handled by recursion.
    ///
    /// parent_empty_block serves as a way to signal that the parent is a block, but there was no
    /// text before we traversed into the first child (think: + *first child* tail).
    fn region_helper(
        &mut self, node_idx: usize, parent_elements: &[Element], parent_empty_block: bool,
    ) {
        let node = &self.ast.nodes[node_idx];
        let mut elements = Vec::from(parent_elements);
        elements.push(node.element.clone());

        let is_block = node.element.is_block() || parent_empty_block;

        if node.children.is_empty() {
            self.styled.extend(self.cursor_split(StyledChunk {
                block_start: is_block,
                range: node.range.clone(),
                elements,
            }));
            return;
        }

        let head_range =
            Range { start: node.range.start, end: self.ast.nodes[node.children[0]].range.start };

        let tail_range = Range {
            start: self.ast.nodes[*node.children.last().unwrap()].range.end,
            end: node.range.end,
        };

        if !head_range.is_empty() {
            self.styled.extend(self.cursor_split(StyledChunk {
                block_start: is_block,
                range: head_range.clone(),
                elements: elements.clone(),
            }));
        }

        for index in 0..self.ast.nodes[node_idx].children.len() {
            let child_idx = self.ast.nodes[node_idx].children[index];
            let first_index = index == 0;
            self.region_helper(
                child_idx,
                &elements,
                is_block && first_index && head_range.is_empty(),
            );
            // collect any regions in between children
            let node = &self.ast.nodes[node_idx];
            let child = &self.ast.nodes[child_idx];
            if let Some(&next_idx) = node.children.get(index + 1) {
                let next = &self.ast.nodes[next_idx];
                let range = Range { start: child.range.end, end: next.range.start };
                // only collect if non empty & not between items
                // todo: this may not be needed anymore because of `look_back_whitespace(...)`
                if !(range.is_empty() || child.element == Item && next.element == Item) {
                    self.styled.extend(self.cursor_split(StyledChunk {
                        block_start: elements == vec![Element::Document],
                        range,
                        elements: elements.clone(),
                    }));
                }
            }
        }

        if !tail_range.is_empty() {
            self.styled.extend(self.cursor_split(StyledChunk {
                block_start: elements == vec![Element::Document],
                range: tail_range,
                elements,
            }));
        }
    }

    fn cursor_split(&self, styled: StyledChunk) -> Vec<StyledChunk> {
        if let Some(cursor_selection) = self.selection_range_bytes() {
            // split region based on cursor selection
            let mut result = Vec::new();
            let mut block_start = styled.block_start;
            if styled.range.start < cursor_selection.start {
                let mut pre_selection = styled.clone();
                pre_selection.range = Range {
                    start: styled.range.start,
                    end: min(styled.range.end, cursor_selection.start),
                };
                pre_selection.block_start = block_start;
                block_start = false;

                result.push(pre_selection);
            }
            if cursor_selection.start < styled.range.end
                && styled.range.start < cursor_selection.end
            {
                let mut in_selection = styled.clone();
                in_selection.range = Range {
                    start: max(styled.range.start, cursor_selection.start),
                    end: min(styled.range.end, cursor_selection.end),
                };
                in_selection.elements.push(Element::Selection);
                in_selection.block_start = block_start;
                block_start = false;

                result.push(in_selection);
            }
            if cursor_selection.end < styled.range.end {
                let mut post_selection = styled.clone();
                post_selection.range = Range {
                    start: max(styled.range.start, cursor_selection.end),
                    end: styled.range.end,
                };
                post_selection.block_start = block_start;

                result.push(post_selection);
            }
            result
        } else {
            // single region
            vec![styled]
        }
    }

    pub fn populate_styled(&mut self) {
        self.styled.clear();
        self.region_helper(self.ast.root, &[], false);
    }

    pub fn print_styled(&self) {
        for styled in &self.styled {
            println!("elements: {:?}", styled.elements);
            println!("range: {}", &self.raw[styled.range.start.0..styled.range.end.0]);
            if styled.block_start {
                println!("start")
            }
            println!();
        }
    }
}
