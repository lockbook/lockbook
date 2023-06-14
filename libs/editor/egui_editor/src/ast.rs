use crate::buffer::SubBuffer;
use crate::element::Element;
use crate::offset_types::{DocCharOffset, RangeExt};
use crate::Editor;
use pulldown_cmark::{Event, OffsetIter, Options, Parser};

#[derive(Default, Debug)]
pub struct Ast {
    pub nodes: Vec<AstNode>,
    pub root: usize,
}

#[derive(Default, Debug)]
pub struct AstNode {
    pub element: Element,
    pub range: (DocCharOffset, DocCharOffset),
    pub children: Vec<usize>,
}

pub fn calc(buffer: &SubBuffer) -> Ast {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    let parser = Parser::new_ext(&buffer.text, options);
    let mut result = Ast {
        nodes: vec![AstNode::new(
            Element::Document,
            (0.into(), buffer.segs.last_cursor_position()),
        )],
        root: 0,
    };
    result.push_children(result.root, &mut parser.into_offset_iter(), buffer);
    result
}

impl Ast {
    pub fn ast_node_at_char(&self, offset: DocCharOffset) -> usize {
        let mut chosen = 0;
        let mut smallest_chosen_ast_range = usize::MAX;

        for i in 0..self.nodes.len() {
            if self.nodes[i].range.contains(offset)
                && self.nodes[i].range.len().0 < smallest_chosen_ast_range
            {
                chosen = i;
                smallest_chosen_ast_range = self.nodes[i].range.len().0;
            }
        }

        chosen
    }

    fn push_children(&mut self, current_idx: usize, iter: &mut OffsetIter, buffer: &SubBuffer) {
        while let Some((event, range)) = iter.next() {
            let mut range = buffer
                .segs
                .range_to_char((range.start.into(), range.end.into()));
            match event {
                Event::Start(new_child) => {
                    if let Some(new_child) = Element::from_tag(new_child) {
                        if new_child == Element::Item {
                            range = (
                                range.start() - Self::look_back_whitespace(buffer, range.start()),
                                range.end() - Self::look_back_newlines(buffer, range.end()),
                            );
                        }
                        if new_child == Element::CodeBlock {
                            range = (
                                range.start(),
                                range.end() + Self::capture_codeblock_newline(buffer, range),
                            );
                        }

                        let new_child_idx =
                            self.push_child(current_idx, AstNode::new(new_child, range));
                        self.push_children(new_child_idx, iter, buffer);
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
    fn look_back_whitespace(buffer: &SubBuffer, start: DocCharOffset) -> usize {
        let mut modification = 0;
        loop {
            if start < modification + 1 {
                break;
            }
            let location = start - (modification + 1);

            let white_maybe = &buffer[(location, location + 1)];
            if white_maybe == " " || white_maybe == "\t" {
                modification += 1;
            } else {
                break;
            }
        }
        modification
    }

    // release this many newlines from the end of a list item
    fn look_back_newlines(buffer: &SubBuffer, end: DocCharOffset) -> usize {
        let mut modification = 0;
        loop {
            if end < modification + 1 {
                break;
            }
            let location = end - (modification + 1);

            if &buffer[(location, location + 1)] == "\n" {
                modification += 1;
            } else {
                break;
            }
        }

        // leave up to one newline
        modification = modification.saturating_sub(1);

        modification
    }

    fn capture_codeblock_newline(
        buffer: &SubBuffer, range: (DocCharOffset, DocCharOffset),
    ) -> usize {
        if buffer.segs.last_cursor_position() < range.end() + 1 {
            return 0;
        }

        if &buffer[(range.start(), range.start() + 1)] != "`" {
            return 0;
        }

        if &buffer[(range.end(), range.end() + 1)] == "\n" {
            return 1;
        }

        0
    }

    pub fn print(&self, buffer: &SubBuffer) {
        Self::print_recursive(self, self.root, buffer, "");
    }

    fn print_recursive(ast: &Ast, node_idx: usize, buffer: &SubBuffer, prefix: &str) {
        let node = &ast.nodes[node_idx];
        let prefix = format!("{}[{:?} {:?}]", prefix, node_idx, node.element);

        if node.children.is_empty() {
            println!(
                "{}: {:?}..{:?} ({:?})",
                prefix,
                node.range.start(),
                node.range.end(),
                &buffer[node.range],
            );
        } else {
            let head = (node.range.start(), ast.nodes[node.children[0]].range.start());
            if !head.is_empty() {
                println!("{}: {:?}..{:?} ({:?})", prefix, head.start(), head.end(), &buffer[head]);
            }
            for child_idx in 0..node.children.len() {
                let child = node.children[child_idx];
                Self::print_recursive(ast, child, buffer, &prefix);
                if child_idx != node.children.len() - 1 {
                    let next_child = node.children[child_idx + 1];
                    let mid = (ast.nodes[child].range.end(), ast.nodes[next_child].range.start());
                    if !mid.is_empty() {
                        println!(
                            "{}: {:?}..{:?} ({:?})",
                            prefix,
                            mid.start(),
                            mid.end(),
                            &buffer[mid]
                        );
                    }
                }
            }
            let tail = (
                ast.nodes[node.children[node.children.len() - 1]]
                    .range
                    .end(),
                node.range.end(),
            );
            if tail.is_empty() {
                println!("{}: {:?}..{:?} ({:?})", prefix, tail.start(), tail.end(), &buffer[tail]);
            }
        }
    }
}

impl AstNode {
    pub fn new(element: Element, range: (DocCharOffset, DocCharOffset)) -> Self {
        Self { element, range, children: vec![] }
    }
}

impl Editor {
    pub fn print_ast(&self) {
        println!("ast:");
        self.ast.print(&self.buffer.current);
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
