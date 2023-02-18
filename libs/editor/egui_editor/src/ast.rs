use crate::buffer::SubBuffer;
use crate::element::Element;
use crate::offset_types::DocByteOffset;
use crate::Editor;
use pulldown_cmark::{Event, OffsetIter, Options, Parser};
use std::ops::Range;

#[derive(Default, Debug)]
pub struct Ast {
    pub nodes: Vec<AstNode>,
    pub root: usize,
}

#[derive(Default, Debug)]
pub struct AstNode {
    pub element: Element,
    pub range: Range<DocByteOffset>,
    pub children: Vec<usize>,
}

pub fn calc(buffer: &SubBuffer) -> Ast {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    let parser = Parser::new_ext(&buffer.text, options);
    let mut result = Ast {
        nodes: vec![AstNode::new(
            Element::Document,
            Range { start: DocByteOffset(0), end: DocByteOffset(buffer.len()) },
        )],
        root: 0,
    };
    result.push_children(result.root, &mut parser.into_offset_iter(), &buffer.text);
    result
}

impl Ast {
    fn push_children(&mut self, current_idx: usize, iter: &mut OffsetIter, raw: &str) {
        while let Some((event, range)) = iter.next() {
            let mut range =
                Range { start: DocByteOffset(range.start), end: DocByteOffset(range.end) };
            match event {
                Event::Start(new_child) => {
                    if let Some(new_child) = Element::from_tag(new_child) {
                        if new_child == Element::Item {
                            range.start -= Self::look_back_whitespace(raw, range.start);
                            range.end -= Self::look_back_newlines(raw, range.end);
                        }
                        if new_child == Element::CodeBlock {
                            range.end +=
                                Self::capture_codeblock_newline(raw, range.start, range.end);
                        }

                        let new_child_idx =
                            self.push_child(current_idx, AstNode::new(new_child, range));
                        self.push_children(new_child_idx, iter, raw);
                    }
                }
                Event::Code(_) => {
                    self.push_child(current_idx, AstNode::new(Element::InlineCode, range));
                }
                Event::End(done) => {
                    if let Some(done) = Element::from_tag(done) {
                        if done == self.nodes[current_idx].element {
                            break;
                        }
                    }
                }
                _ => {} // todo: there are some interesting events ignored (rules, tables, etc)
            }
        }
    }

    fn push_child(&mut self, parent_idx: usize, node: AstNode) -> usize {
        let new_child_idx = self.nodes.len();
        self.nodes.push(node);
        self.nodes[parent_idx].children.push(new_child_idx);
        new_child_idx
    }

    // capture this many spaces or tabs from before a list item
    fn look_back_whitespace(raw: &str, start: DocByteOffset) -> usize {
        let mut modification = 0;
        loop {
            if start < modification + 1 {
                break;
            }
            let location = start - (modification + 1);

            let white_maybe = &raw[location.0..location.0 + 1];
            if white_maybe == " " || white_maybe == "\t" {
                modification += 1;
            } else {
                break;
            }
        }
        modification
    }

    // release this many newlines from the end of a list item
    fn look_back_newlines(raw: &str, end: DocByteOffset) -> usize {
        let mut modification = 0;
        loop {
            if end < modification + 1 {
                break;
            }
            let location = end - (modification + 1);

            if raw.is_char_boundary(location.0) && &raw[location.0..location.0 + 1] == "\n" {
                modification += 1;
            } else {
                break;
            }
        }

        // leave up to one newline
        modification = modification.saturating_sub(1);

        modification
    }

    fn capture_codeblock_newline(raw: &str, start: DocByteOffset, end: DocByteOffset) -> usize {
        if raw.len() < end.0 + 1 {
            return 0;
        }

        if &raw[start.0..start.0 + 1] != "`" {
            return 0;
        }

        if &raw[end.0..end.0 + 1] == "\n" {
            return 1;
        }

        0
    }

    pub fn print(&self, raw: &str) {
        Self::print_recursive(self, self.root, raw, "");
    }

    fn print_recursive(ast: &Ast, node_idx: usize, raw: &str, prefix: &str) {
        let node = &ast.nodes[node_idx];
        let prefix = format!("{}[{:?} {:?}]", prefix, node_idx, node.element);

        if node.children.is_empty() {
            println!(
                "{}: {:?}..{:?} ({:?})",
                prefix,
                node.range.start.0,
                node.range.end.0,
                &raw[node.range.start.0..node.range.end.0]
            );
        } else {
            let head_range =
                Range { start: node.range.start.0, end: ast.nodes[node.children[0]].range.start.0 };
            if !head_range.is_empty() {
                println!(
                    "{}: {:?}..{:?} ({:?})",
                    prefix,
                    head_range.start,
                    head_range.end,
                    &raw[head_range.start..head_range.end]
                );
            }
            for child_idx in 0..node.children.len() {
                let child = node.children[child_idx];
                Self::print_recursive(ast, child, raw, &prefix);
                if child_idx != node.children.len() - 1 {
                    let next_child = node.children[child_idx + 1];
                    let mid_range = Range {
                        start: ast.nodes[child].range.end.0,
                        end: ast.nodes[next_child].range.start.0,
                    };
                    if !mid_range.is_empty() {
                        println!(
                            "{}: {:?}..{:?} ({:?})",
                            prefix,
                            mid_range.start,
                            mid_range.end,
                            &raw[mid_range.start..mid_range.end]
                        );
                    }
                }
            }
            let tail_range = Range {
                start: ast.nodes[node.children[node.children.len() - 1]]
                    .range
                    .end
                    .0,
                end: node.range.end.0,
            };
            if !tail_range.is_empty() {
                println!(
                    "{}: {:?}..{:?} ({:?})",
                    prefix,
                    tail_range.start,
                    tail_range.end,
                    &raw[tail_range.start..tail_range.end]
                );
            }
        }
    }
}

impl AstNode {
    pub fn new(element: Element, range: Range<DocByteOffset>) -> Self {
        Self { element, range, children: vec![] }
    }
}

impl Editor {
    pub fn print_ast(&self) {
        println!("ast:");
        self.ast.print(&self.buffer.current.text);
    }
}

// grievances with pullmark:
// 1. inconsistent block behavior: code blocks do not terminate with a newline, but headings and
//    other elements do (TEST_MARKDOWN_13 vs TEST_MARKDOWN_25)
// 2. inconsistent code block behavior, a code block that is defined with spaces in front (rather
//    than by a code fence) begins at the first character after the spaces, but the space characters
//    are not absorbed anywhere else. And this code block includes a \n at the end unlike the code
//    fence block
// 3. the indentation (whitespace) at the start of an item is not part of the item
// 4. a \n\n at the end of an item remains part of that item even if it's a \n\ntest
//
// These things are either going to serve as motivation for a custom editor down the road, or an
// explanation for strange things like look_back_whitespsace
