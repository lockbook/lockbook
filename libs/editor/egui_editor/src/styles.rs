use crate::appearance::Appearance;
use crate::ast::Ast;
use crate::editor::Editor;
use crate::element::Element;
use crate::input::cursor::Cursor;
use crate::offset_types::{DocCharOffset, RangeExt};
use egui::TextFormat;
use std::cmp::{max, min};

#[derive(Debug, Clone, PartialEq)]
pub struct StyleInfo {
    pub block_start: bool,
    pub range: (DocCharOffset, DocCharOffset),
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

/// Traverse an AST and determine style info, which is a set of text ranges, each with a stack of
/// AST elements that they are in and an indicator of whether they are the start of a block (galley)
pub fn calc(ast: &Ast, cursor: Cursor) -> Vec<StyleInfo> {
    let mut styles = Vec::new();
    calc_recursive(ast, cursor, ast.root, &[], true, &mut styles);
    styles
}

/// recursive implementation of calc(); returns whether a block was started
fn calc_recursive(
    ast: &Ast, cursor: Cursor, node_idx: usize, parent_elements: &[Element],
    should_start_block_for_ancestor: bool, styles: &mut Vec<StyleInfo>,
) -> bool {
    let node = &ast.nodes[node_idx];
    let mut elements = Vec::from(parent_elements);
    elements.push(node.element.clone());

    let should_start_block = node.element == Element::Item || should_start_block_for_ancestor;
    let children_start_blocks = elements == [Element::Document];
    let mut did_start_block = false;

    // if this is a leaf node, just emit the style for the whole range
    if node.children.is_empty() {
        let style = StyleInfo {
            block_start: children_start_blocks || (should_start_block && !did_start_block),
            range: node.range,
            elements,
        };
        if style.block_start {
            did_start_block = true;
        }
        styles.extend(cursor_split(cursor, style));
        return did_start_block;
    }

    // emit style for text before first child
    let head_range = (node.range.start(), ast.nodes[node.children[0]].range.start());
    if !head_range.is_empty() {
        let style = StyleInfo {
            block_start: children_start_blocks || (should_start_block && !did_start_block),
            range: head_range,
            elements: elements.clone(),
        };
        if style.block_start {
            did_start_block = true;
        }
        styles.extend(cursor_split(cursor, style));
    }

    // emit style for children and text between children
    for index in 0..ast.nodes[node_idx].children.len() {
        // emit style for children
        let child_idx = ast.nodes[node_idx].children[index];
        did_start_block |= calc_recursive(
            ast,
            cursor,
            child_idx,
            &elements,
            children_start_blocks || (should_start_block && !did_start_block),
            styles,
        );

        // emit style for text between children
        let node = &ast.nodes[node_idx];
        let child = &ast.nodes[child_idx];
        if let Some(&next_idx) = node.children.get(index + 1) {
            let style = StyleInfo {
                block_start: children_start_blocks || (should_start_block && !did_start_block),
                range: (child.range.end(), ast.nodes[next_idx].range.start()),
                elements: elements.clone(),
            };
            if !style.range.is_empty() {
                if style.block_start {
                    did_start_block = true;
                }
                styles.extend(cursor_split(cursor, style));
            }
        }
    }

    // emit style for text after last child
    let tail_range = (ast.nodes[*node.children.last().unwrap()].range.end(), node.range.end());
    if !tail_range.is_empty() {
        let style = StyleInfo {
            block_start: children_start_blocks || (should_start_block && !did_start_block),
            range: tail_range,
            elements: elements.clone(),
        };
        if style.block_start {
            did_start_block = true;
        }
        styles.extend(cursor_split(cursor, style));
    }

    did_start_block
}

fn cursor_split(cursor: Cursor, style: StyleInfo) -> Vec<StyleInfo> {
    if let Some(cursor_selection) = cursor.selection().or(cursor.mark_highlight()) {
        // split region based on cursor selection
        let mut result = Vec::new();
        let mut block_start = style.block_start;
        if style.range.start() < cursor_selection.start {
            let mut pre_selection = style.clone();
            pre_selection.range =
                (style.range.start(), min(style.range.end(), cursor_selection.start));
            pre_selection.block_start = block_start;
            block_start = false;

            result.push(pre_selection);
        }
        if cursor_selection.start < style.range.end() && style.range.start() < cursor_selection.end
        {
            let mut in_selection = style.clone();
            in_selection.range = (
                max(style.range.start(), cursor_selection.start),
                min(style.range.end(), cursor_selection.end),
            );
            in_selection.elements.push(Element::Selection);
            in_selection.block_start = block_start;
            block_start = false;

            result.push(in_selection);
        }
        if cursor_selection.end < style.range.end() {
            let mut post_selection = style.clone();
            post_selection.range =
                (max(style.range.start(), cursor_selection.end), style.range.end());
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
        println!("styles:");
        for style in &self.styles {
            println!(
                "{}{:?}: {:?}..{:?} ({:?})",
                if style.block_start { "block: " } else { "       " },
                style.elements,
                style.range.start().0,
                style.range.end().0,
                &self.buffer.current.text[style.range.start().0..style.range.end().0],
            );
        }
    }
}

#[cfg(test)]
mod test {
    use crate::ast::{Ast, AstNode};
    use crate::element::Element;
    use crate::styles::{calc, StyleInfo};
    use pulldown_cmark::HeadingLevel::H1;

    #[test]
    fn calc_title_with_newline() {
        let nodes = vec![
            AstNode { element: Element::Document, range: (0.into(), 9.into()), children: vec![1] },
            AstNode {
                element: Element::Heading(H1),
                range: (0.into(), 8.into()),
                children: vec![],
            },
        ];
        let ast = Ast { nodes, root: 0 };
        let expected_styles: Vec<StyleInfo> = vec![
            StyleInfo {
                block_start: true,
                range: (0.into(), 8.into()),
                elements: vec![Element::Document, Element::Heading(H1)],
            },
            StyleInfo {
                block_start: true,
                range: (8.into(), 9.into()),
                elements: vec![Element::Document],
            },
        ];

        let actual_styles = calc(&ast, 0.into());

        assert_eq!(actual_styles, expected_styles);
    }

    #[test]
    fn calc_nested_bullet_with_code() {
        let nodes = vec![
            AstNode { element: Element::Document, range: (0.into(), 14.into()), children: vec![1] },
            AstNode { element: Element::Item, range: (0.into(), 14.into()), children: vec![2] },
            AstNode { element: Element::Item, range: (6.into(), 14.into()), children: vec![3] },
            AstNode {
                element: Element::InlineCode,
                range: (9.into(), 14.into()),
                children: vec![],
            },
        ];
        let ast = Ast { nodes, root: 0 };
        let expected_styles: Vec<StyleInfo> = vec![
            StyleInfo {
                block_start: true,
                range: (0.into(), 6.into()),
                elements: vec![Element::Document, Element::Item],
            },
            StyleInfo {
                block_start: true,
                range: (6.into(), 9.into()),
                elements: vec![Element::Document, Element::Item, Element::Item],
            },
            StyleInfo {
                block_start: false,
                range: (9.into(), 14.into()),
                elements: vec![
                    Element::Document,
                    Element::Item,
                    Element::Item,
                    Element::InlineCode,
                ],
            },
        ];

        let actual_styles = calc(&ast, 0.into());

        assert_eq!(actual_styles, expected_styles);
    }

    #[test]
    fn calc_bullets_with_intervening_newline() {
        let nodes = vec![
            AstNode {
                element: Element::Document,
                range: (0.into(), 12.into()),
                children: vec![1, 3],
            },
            AstNode { element: Element::Item, range: (0.into(), 6.into()), children: vec![2] },
            AstNode { element: Element::Paragraph, range: (2.into(), 6.into()), children: vec![] },
            AstNode { element: Element::Item, range: (7.into(), 12.into()), children: vec![4] },
            AstNode { element: Element::Paragraph, range: (9.into(), 12.into()), children: vec![] },
        ];
        let ast = Ast { nodes, root: 0 };
        let expected_styles: Vec<StyleInfo> = vec![
            StyleInfo {
                block_start: true,
                range: (0.into(), 2.into()),
                elements: vec![Element::Document, Element::Item],
            },
            StyleInfo {
                block_start: false,
                range: (2.into(), 6.into()),
                elements: vec![Element::Document, Element::Item, Element::Paragraph],
            },
            StyleInfo {
                block_start: true,
                range: (6.into(), 7.into()),
                elements: vec![Element::Document],
            },
            StyleInfo {
                block_start: true,
                range: (7.into(), 9.into()),
                elements: vec![Element::Document, Element::Item],
            },
            StyleInfo {
                block_start: false,
                range: (9.into(), 12.into()),
                elements: vec![Element::Document, Element::Item, Element::Paragraph],
            },
        ];

        let actual_styles = calc(&ast, 0.into());

        assert_eq!(actual_styles, expected_styles);
    }
}
