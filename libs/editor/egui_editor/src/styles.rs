use crate::appearance::Appearance;
use crate::ast::Ast;
use crate::editor::Editor;
use crate::element::Element;
use crate::offset_types::DocByteOffset;
use egui::TextFormat;
use std::cmp::{max, min};
use std::ops::Range;

#[derive(Debug, Clone)]
pub struct StyleInfo {
    pub block_start: bool,
    pub range: Range<DocByteOffset>,
    pub elements: Vec<Element>, // maybe a good tiny_vec candidate
}

impl StyleInfo {
    pub fn item_count(&self) -> usize {
        self.elements
            .iter()
            .filter(|el| matches!(el, Element::Item))
            .count()
    }

    pub fn text_format(&self, visual: &Appearance) -> TextFormat {
        let mut text_format = TextFormat::default();
        for element in &self.elements {
            element.apply_style(&mut text_format, visual);
        }

        text_format
    }
}

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
pub fn calc(ast: &Ast, selection: &Option<Range<DocByteOffset>>) -> Vec<StyleInfo> {
    let mut styles = Vec::new();
    calc_recursive(ast, selection, ast.root, &[], false, &mut styles);
    styles
}

fn calc_recursive(
    ast: &Ast, selection: &Option<Range<DocByteOffset>>, node_idx: usize,
    parent_elements: &[Element], parent_empty_block: bool, styles: &mut Vec<StyleInfo>,
) {
    let node = &ast.nodes[node_idx];
    let mut elements = Vec::from(parent_elements);
    elements.push(node.element.clone());

    let is_block = node.element.is_block() || parent_empty_block;

    if node.children.is_empty() {
        styles.extend(cursor_split(
            selection,
            StyleInfo { block_start: is_block, range: node.range.clone(), elements },
        ));
        return;
    }

    let head_range =
        Range { start: node.range.start, end: ast.nodes[node.children[0]].range.start };

    let tail_range =
        Range { start: ast.nodes[*node.children.last().unwrap()].range.end, end: node.range.end };

    if !head_range.is_empty() {
        styles.extend(cursor_split(
            selection,
            StyleInfo {
                block_start: is_block,
                range: head_range.clone(),
                elements: elements.clone(),
            },
        ));
    }

    for index in 0..ast.nodes[node_idx].children.len() {
        let child_idx = ast.nodes[node_idx].children[index];
        let first_index = index == 0;
        calc_recursive(
            ast,
            selection,
            child_idx,
            &elements,
            is_block && first_index && head_range.is_empty(),
            styles,
        );
        // collect any regions in between children
        let node = &ast.nodes[node_idx];
        let child = &ast.nodes[child_idx];
        if let Some(&next_idx) = node.children.get(index + 1) {
            let next = &ast.nodes[next_idx];
            let range = Range { start: child.range.end, end: next.range.start };
            // only collect if non empty & not between items
            // todo: this may not be needed anymore because of `look_back_whitespace(...)`
            if !(range.is_empty()
                || child.element == Element::Item && next.element == Element::Item)
            {
                styles.extend(cursor_split(
                    selection,
                    StyleInfo {
                        block_start: elements == vec![Element::Document],
                        range,
                        elements: elements.clone(),
                    },
                ));
            }
        }
    }

    if !tail_range.is_empty() {
        styles.extend(cursor_split(
            selection,
            StyleInfo {
                block_start: elements == vec![Element::Document],
                range: tail_range,
                elements,
            },
        ));
    }
}

fn cursor_split(
    selection_range_bytes: &Option<Range<DocByteOffset>>, style: StyleInfo,
) -> Vec<StyleInfo> {
    if let Some(cursor_selection) = selection_range_bytes {
        // split region based on cursor selection
        let mut result = Vec::new();
        let mut block_start = style.block_start;
        if style.range.start < cursor_selection.start {
            let mut pre_selection = style.clone();
            pre_selection.range = Range {
                start: style.range.start,
                end: min(style.range.end, cursor_selection.start),
            };
            pre_selection.block_start = block_start;
            block_start = false;

            result.push(pre_selection);
        }
        if cursor_selection.start < style.range.end && style.range.start < cursor_selection.end {
            let mut in_selection = style.clone();
            in_selection.range = Range {
                start: max(style.range.start, cursor_selection.start),
                end: min(style.range.end, cursor_selection.end),
            };
            in_selection.elements.push(Element::Selection);
            in_selection.block_start = block_start;
            block_start = false;

            result.push(in_selection);
        }
        if cursor_selection.end < style.range.end {
            let mut post_selection = style.clone();
            post_selection.range =
                Range { start: max(style.range.start, cursor_selection.end), end: style.range.end };
            post_selection.block_start = block_start;

            result.push(post_selection);
        }
        result
    } else {
        // single region
        vec![style]
    }
}

impl Editor {
    pub fn print_styles(&self) {
        for style in &self.styles {
            println!("elements: {:?}", style.elements);
            println!("range: {}", &self.buffer.raw[style.range.start.0..style.range.end.0]);
            if style.block_start {
                println!("start")
            }
            println!();
        }
    }
}
