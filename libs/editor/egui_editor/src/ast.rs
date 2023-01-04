use crate::cursor_types::DocByteOffset;
use crate::element::Element;
use pulldown_cmark::{Event, OffsetIter, Options, Parser};
use std::ops::Range;

#[derive(Default, Debug)]
pub struct Ast {
    pub nodes: Vec<ASTNode>,
    pub root: usize,
}

#[derive(Default, Debug)]
pub struct ASTNode {
    pub element: Element,
    pub range: Range<DocByteOffset>,
    pub children: Vec<usize>,
}

impl Ast {
    pub fn parse(raw: &str) -> Self {
        let mut options = Options::empty();
        options.insert(Options::ENABLE_STRIKETHROUGH);
        let parser = Parser::new_ext(raw, options);
        let mut result = Self {
            nodes: vec![ASTNode::new(
                Element::Document,
                Range { start: DocByteOffset(0), end: DocByteOffset(raw.len()) },
            )],
            root: 0,
        };
        result.push_children(result.root, &mut parser.into_offset_iter(), raw);
        result
    }

    fn push_children(&mut self, current_idx: usize, iter: &mut OffsetIter, raw: &str) {
        while let Some((event, range)) = iter.next() {
            let mut range =
                Range { start: DocByteOffset(range.start), end: DocByteOffset(range.end) };
            match event {
                Event::Start(new_child) => {
                    if let Some(new_child) = Element::from_tag(new_child) {
                        if new_child == Element::Item {
                            range.start -= Self::look_back_whitespace(raw, range.start);
                        }
                        if new_child == Element::CodeBlock {
                            range.end +=
                                Self::capture_codeblock_newline(raw, range.start, range.end);
                        }

                        let new_child_idx =
                            self.push_child(current_idx, ASTNode::new(new_child, range));
                        self.push_children(new_child_idx, iter, raw);
                    }
                }
                Event::Code(_) => {
                    self.push_child(current_idx, ASTNode::new(Element::InlineCode, range));
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

    fn push_child(&mut self, parent_idx: usize, node: ASTNode) -> usize {
        let new_child_idx = self.nodes.len();
        self.nodes.push(node);
        self.nodes[parent_idx].children.push(new_child_idx);
        new_child_idx
    }

    fn look_back_whitespace(s: &str, index: DocByteOffset) -> usize {
        if index == 0 {
            return 0;
        };

        let mut modification = 0;
        loop {
            let location = index - (modification + 1);
            let range = Range { start: location, end: location + 1 };

            if location + 1 > s.len() - 1 {
                break;
            }

            let white_maybe = &s[range.start.0..range.end.0];
            if white_maybe == " " || white_maybe == "\t" {
                modification += 1;
            } else {
                break;
            }
        }
        modification
    }

    fn capture_codeblock_newline(s: &str, start: DocByteOffset, end: DocByteOffset) -> usize {
        if s.len() < end.0 + 1 {
            return 0;
        }

        if &s[start.0..start.0 + 1] != "`" {
            return 0;
        }

        if &s[end.0..end.0 + 1] == "\n" {
            return 1;
        }

        0
    }
}

impl ASTNode {
    pub fn new(element: Element, range: Range<DocByteOffset>) -> Self {
        Self { element, range, children: vec![] }
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
