use crate::tab::markdown_editor::bounds::{AstTextRanges, RangesExt};
use crate::tab::markdown_editor::buffer::SubBuffer;
use crate::tab::markdown_editor::input::cursor::Cursor;
use crate::tab::markdown_editor::layouts::Annotation;
use crate::tab::markdown_editor::offset_types::{
    DocCharOffset, RangeExt, RangeIterExt, RelCharOffset,
};
use crate::tab::markdown_editor::style::{
    BlockNode, BlockNodeType, InlineNode, ListItem, MarkdownNode, MarkdownNodeType,
};
use crate::tab::markdown_editor::Editor;
use pulldown_cmark::{Event, HeadingLevel, LinkType, OffsetIter, Options, Parser, Tag};

#[derive(Default, Debug, PartialEq)]
pub struct Ast {
    pub nodes: Vec<AstNode>,
    pub root: usize,
}

#[derive(Default, Debug, PartialEq)]
pub struct AstNode {
    /// Type of markdown node e.g. heading and relevant information e.g. heading level
    pub node_type: MarkdownNode,

    /// Range of source text captured
    pub range: (DocCharOffset, DocCharOffset),

    /// Range of source text still rendered after syntax characters are captured/interpreted
    pub text_range: (DocCharOffset, DocCharOffset),

    /// Indexes of sub-nodes in the vector containing this node
    pub children: Vec<usize>,
}

impl AstNode {
    pub fn head_range(&self) -> (DocCharOffset, DocCharOffset) {
        (self.range.0, self.text_range.0)
    }

    pub fn tail_range(&self) -> (DocCharOffset, DocCharOffset) {
        (self.text_range.1, self.range.1)
    }
}

pub fn calc(buffer: &SubBuffer) -> Ast {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    let parser = Parser::new_ext(&buffer.text, options);
    let mut result = Ast {
        nodes: vec![AstNode::new(
            MarkdownNode::Document,
            (0.into(), buffer.segs.last_cursor_position()),
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
            if self.nodes[i].range.contains_inclusive(offset)
                && self.nodes[i].range.len().0 < smallest_chosen_ast_range
            {
                chosen = i;
                smallest_chosen_ast_range = self.nodes[i].range.len().0;
            }
        }

        chosen
    }

    pub fn parent(&self, node_idx: usize) -> Option<usize> {
        self.nodes
            .iter()
            .enumerate()
            .find(|(_, n)| n.children.contains(&node_idx))
            .map(|(idx, _)| idx)
    }

    fn push_children(&mut self, current_idx: usize, iter: &mut OffsetIter, buffer: &SubBuffer) {
        let mut skipped = 0;
        while let Some((event, range)) = iter.next() {
            let range = buffer
                .segs
                .range_to_char((range.start.into(), range.end.into()));
            match event {
                Event::Start(child_tag) => {
                    let new_child_node = match child_tag {
                        Tag::Paragraph => MarkdownNode::Paragraph,
                        Tag::Heading(level, _, _) => MarkdownNode::Block(BlockNode::Heading(level)),
                        Tag::BlockQuote => MarkdownNode::Block(BlockNode::Quote),
                        Tag::CodeBlock(_) => MarkdownNode::Block(BlockNode::Code),
                        Tag::Item => {
                            let item_type = Self::item_type(&buffer[range]);
                            let mut indent_level = 0;
                            let mut ancestor_idx = current_idx;
                            while ancestor_idx != 0 {
                                if matches!(
                                    self.nodes[current_idx].node_type,
                                    MarkdownNode::Block(BlockNode::ListItem(..))
                                ) {
                                    indent_level += 1;
                                }

                                // advance to parent
                                ancestor_idx = self
                                    .nodes
                                    .iter()
                                    .enumerate()
                                    .find(|(_, n)| n.children.contains(&ancestor_idx))
                                    .map(|(idx, _)| idx)
                                    .unwrap_or_default();
                            }
                            MarkdownNode::Block(BlockNode::ListItem(item_type, indent_level))
                        }
                        Tag::Emphasis => MarkdownNode::Inline(InlineNode::Italic),
                        Tag::Strong => MarkdownNode::Inline(InlineNode::Bold),
                        Tag::Strikethrough => MarkdownNode::Inline(InlineNode::Strikethrough),
                        Tag::Link(l, u, t) => {
                            MarkdownNode::Inline(InlineNode::Link(l, u.to_string(), t.to_string()))
                        }
                        Tag::Image(l, u, t) => {
                            MarkdownNode::Inline(InlineNode::Image(l, u.to_string(), t.to_string()))
                        }
                        _ => {
                            skipped += 1;
                            continue;
                        }
                    };
                    if let Some(new_child_idx) =
                        self.push_child(current_idx, new_child_node, range, buffer)
                    {
                        self.push_children(new_child_idx, iter, buffer);
                    }
                }
                Event::Code(_) => {
                    self.push_child(
                        current_idx,
                        MarkdownNode::Inline(InlineNode::Code),
                        range,
                        buffer,
                    );
                }
                Event::Rule => {
                    self.push_child(
                        current_idx,
                        MarkdownNode::Block(BlockNode::Rule),
                        range,
                        buffer,
                    );
                }
                Event::End(_) => {
                    if skipped == 0 {
                        break;
                    } else {
                        skipped -= 1;
                    }
                }
                _ => {} // todo: there are some interesting events ignored (rules, tables, etc)
            }
        }
    }

    fn item_type(text: &str) -> ListItem {
        let text = text.trim_start();
        if text.starts_with("+ [ ]") || text.starts_with("* [ ]") || text.starts_with("- [ ]") {
            ListItem::Todo(false)
        } else if text.starts_with("+ [x]")
            || text.starts_with("* [x]")
            || text.starts_with("- [x]")
        {
            ListItem::Todo(true)
        } else if let Some(prefix) = text.split('.').next() {
            if let Ok(num) = prefix.parse::<usize>() {
                ListItem::Numbered(num)
            } else {
                ListItem::Bulleted // default to bullet
            }
        } else {
            ListItem::Bulleted // default to bullet
        }
    }

    fn push_child(
        &mut self, parent_idx: usize, mut markdown_node: MarkdownNode,
        cmark_range: (DocCharOffset, DocCharOffset), buffer: &SubBuffer,
    ) -> Option<usize> {
        // assumption: whitespace-only nodes have no children
        if buffer[cmark_range].trim().is_empty() {
            return None;
        }

        let range = {
            let mut range = cmark_range;

            // trim trailing whitespace from range
            // operations that adjust styles will not add or remove trailing whitespace
            range.1 -= buffer[cmark_range].len() - buffer[cmark_range].trim_end().len();

            // capture leading whitespace for list items and code blocks (affects non-fenced code blocks only)
            if matches!(
                markdown_node,
                MarkdownNode::Block(BlockNode::ListItem(..)) | MarkdownNode::Block(BlockNode::Code)
            ) {
                while range.0 > 0
                    && buffer[(range.0 - 1, range.1)]
                        .starts_with(|c: char| c.is_whitespace() && c != '\n')
                {
                    range.0 -= 1;
                }
            }

            // capture up to one trailing space for list items and headings
            if matches!(
                markdown_node,
                MarkdownNode::Block(BlockNode::ListItem(..))
                    | MarkdownNode::Block(BlockNode::Heading(..))
            ) && range.1 < buffer.segs.last_cursor_position()
                && buffer[(range.0, range.1 + 1)].ends_with(' ')
            {
                range.1 += 1;
            }

            // capture up to one trailing newline for rules
            if markdown_node.node_type() == MarkdownNodeType::Block(BlockNodeType::Rule)
                && range.1 < buffer.segs.last_cursor_position()
                && buffer[(range.0, range.1 + 1)].ends_with('\n')
            {
                range.1 += 1;
            }

            // clamp range to text range of parent
            let parent_text_range = self.nodes[parent_idx].text_range;
            let (min, max) = parent_text_range;
            range.0 = range.0.max(min).min(max);
            range.1 = range.1.max(min).min(max);

            if range.is_empty() {
                return None;
            }

            range
        };

        // trim syntax characters from text range
        // the characters between range.0 and text_range.0 are the head characters
        // the characters between text_range.1 and range.1 are the tail characters
        // the head and tail characters are those that are modified when styles are adjusted
        // assumption: syntax characters are single-byte unicode sequences
        let text_range = {
            let mut text_range = range;
            match markdown_node.clone() {
                MarkdownNode::Block(BlockNode::Heading(h)) => {
                    // # heading
                    let original_text_range_0 = text_range.0;

                    text_range.0 += h as usize;

                    // correct cmark behavior with no space in syntax chars
                    if buffer[text_range].starts_with(' ') {
                        text_range.0 += 1;
                    } else {
                        markdown_node = MarkdownNode::Paragraph;
                        text_range.0 = original_text_range_0;
                    }
                }
                MarkdownNode::Block(BlockNode::Quote) => {
                    // >quote block
                    // > quote block
                    if buffer[text_range].starts_with("> ") {
                        text_range.0 += 2;
                    } else if buffer[text_range].starts_with('>') {
                        text_range.0 += 1;
                    }
                }
                MarkdownNode::Block(BlockNode::Code) => {
                    if (buffer[range].starts_with("```\n") && buffer[range].ends_with("\n```"))
                        || (buffer[range].starts_with("~~~\n") && buffer[range].ends_with("\n~~~"))
                    {
                        /*
                        ```
                        code block
                        ```
                        ~~~
                        code block
                        ~~~
                         */
                        text_range.0 += 3;
                        text_range.1 -= 3;
                    } else {
                        /*
                            code block
                        */
                    }
                    if text_range.1 < text_range.0 {
                        /*
                        ```
                        ```
                        ~~~
                        ~~~
                        single newline gets captured in head and not tail
                         */
                        text_range.1 += 1;
                    }
                }
                MarkdownNode::Block(BlockNode::ListItem(item_type, _)) => {
                    // * item
                    //   1. item
                    //     - [ ] item
                    let original_text_range_0 = text_range.0;

                    text_range.0 += buffer[range].len() - buffer[range].trim_start().len();
                    text_range.0 += match item_type {
                        ListItem::Bulleted => 1,
                        ListItem::Numbered(n) => 1 + n.to_string().len(),
                        ListItem::Todo(_) => 5,
                    };

                    // correct cmark behavior with no space in syntax chars
                    if buffer[text_range].starts_with(' ') {
                        text_range.0 += 1;
                    } else {
                        markdown_node = MarkdownNode::Paragraph;
                        text_range.0 = original_text_range_0;
                    }

                    // prevent same-line list item nesting
                    // 1. * nested item
                    let parent_node = &self.nodes[parent_idx];
                    if let MarkdownNode::Block(BlockNode::ListItem(..)) = parent_node.node_type {
                        if !buffer[(parent_node.range.0, range.0)].contains('\n') {
                            markdown_node = MarkdownNode::Paragraph;
                            text_range.0 = original_text_range_0;
                        }
                    }
                }
                MarkdownNode::Block(BlockNode::Rule) => {
                    // ---
                    // ***
                    // ___
                    // -------------------
                    // *******************
                    // ___________________

                    // require a trailing newline
                    if !buffer[range].ends_with('\n') {
                        markdown_node = MarkdownNode::Paragraph;
                    } else {
                        text_range.0 = text_range.1 - 1;
                    }
                }
                MarkdownNode::Inline(InlineNode::Code) => {
                    // `code`
                    text_range.0 += 1;
                    text_range.1 -= 1;
                }
                MarkdownNode::Inline(InlineNode::Bold) => {
                    // __strong__
                    text_range.0 += 2;
                    text_range.1 -= 2;
                }
                MarkdownNode::Inline(InlineNode::Italic) => {
                    // _emphasis_
                    text_range.0 += 1;
                    text_range.1 -= 1;
                }
                MarkdownNode::Inline(InlineNode::Strikethrough) => {
                    // ~strikethrough~ (not strictly markdown spec compliant)
                    if buffer[text_range].starts_with('~') && buffer[text_range].ends_with('~') {
                        text_range.0 += 1;
                        text_range.1 -= 1;
                    }

                    // ~~strikethrough~~
                    if buffer[text_range].starts_with('~') && buffer[text_range].ends_with('~') {
                        text_range.0 += 1;
                        text_range.1 -= 1;
                    }
                }
                MarkdownNode::Inline(InlineNode::Link(LinkType::Inline, url, title)) => {
                    // [title](http://url.com "title")

                    // require url and title
                    if url.is_empty() || buffer[range].starts_with("[]") {
                        markdown_node = MarkdownNode::Paragraph;
                    } else {
                        text_range.0 += 1;
                        text_range.1 -= url.len() + 3;
                        if !title.is_empty() {
                            text_range.1 -= title.len() + 3;
                        }
                    }
                }
                MarkdownNode::Inline(InlineNode::Image(LinkType::Inline, url, title)) => {
                    // ![title](http://url.com)
                    text_range.0 += 2;
                    text_range.1 -= url.len() + 3;
                    if !title.is_empty() {
                        text_range.1 -= title.len() + 3;
                    }
                }
                _ => {}
            };

            text_range
        };

        let ast_node = AstNode::new(markdown_node, range, text_range);
        let new_child_idx = self.nodes.len();
        self.nodes.push(ast_node);
        self.nodes[parent_idx].children.push(new_child_idx);
        Some(new_child_idx)
    }

    pub fn iter_text_ranges(&self) -> AstTextRangeIter {
        AstTextRangeIter {
            ast: self,
            maybe_current_range: Some(AstTextRange {
                range_type: AstTextRangeType::Head,
                range: (0.into(), 0.into()),
                ancestors: vec![0],
            }),
        }
    }

    /// Returns all styles applied to the text just before the given offset if any, or just after when at the document start.
    pub fn styles_at_offset(
        &self, offset: DocCharOffset, bounds: &AstTextRanges,
    ) -> Vec<MarkdownNode> {
        let mut result = Vec::default();
        if let Some(text_range) = bounds.find_containing(offset, true, true).iter().last() {
            let text_range = &bounds[text_range];
            for &ancestor_node_idx in &text_range.ancestors {
                let ancestor_node = &self.nodes[ancestor_node_idx];
                // offset must be in text range (not head/tail syntax chars) to apply
                if ancestor_node.text_range.contains_inclusive(offset) {
                    result.push(ancestor_node.node_type.clone());
                }
            }
        }
        result
    }

    pub fn print(&self, buffer: &SubBuffer) {
        for range in self.iter_text_ranges() {
            println!(
                "{:?} {:?}: {:?}..{:?}\t{:?}",
                range.range_type,
                range
                    .ancestors
                    .iter()
                    .map(|&i| format!("[{:?} {:?}]", i, self.nodes[i].node_type))
                    .collect::<Vec<_>>(),
                range.range.0,
                range.range.1,
                match range.range_type {
                    AstTextRangeType::Head => &buffer[range.range],
                    AstTextRangeType::Text => &buffer[range.range],
                    AstTextRangeType::Tail => &buffer[range.range],
                }
            );
        }
    }
}

impl AstNode {
    pub fn new(
        node: MarkdownNode, range: (DocCharOffset, DocCharOffset),
        text_range: (DocCharOffset, DocCharOffset),
    ) -> Self {
        Self { node_type: node, range, text_range, children: vec![] }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum AstTextRangeType {
    /// Text between `node.range.0` and `node.text_range.0` i.e. leading syntax characters for a node.
    /// Occurs at most once per node.
    Head,

    /// Text between node.text_range.0 and node.text_range.1, excluding ranges captured by child nodes.
    /// Can occur any number of times per node because child nodes slice node text into multiple parts.
    Text,

    /// Text between `node.text_range.1` and `node.range.1` i.e. trailing syntax characters for a node.
    /// Occurs at most once per node.
    Tail,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AstTextRange {
    pub range_type: AstTextRangeType,
    pub range: (DocCharOffset, DocCharOffset),

    /// Indexes of all AST nodes containing this range, ordered from root to leaf.
    pub ancestors: Vec<usize>,
}

impl AstTextRange {
    pub fn node(&self, ast: &Ast) -> MarkdownNode {
        ast.nodes[self.ancestors.last().copied().unwrap_or_default()]
            .node_type
            .clone()
    }

    pub fn annotation(&self, ast: &Ast) -> Option<Annotation> {
        match self.node(ast) {
            MarkdownNode::Block(BlockNode::Heading(HeadingLevel::H1)) => {
                Some(Annotation::HeadingRule)
            }
            MarkdownNode::Inline(InlineNode::Image(link_type, url, title)) => {
                Some(Annotation::Image(link_type, url, title))
            }
            MarkdownNode::Block(BlockNode::ListItem(item_type, indent_level)) => {
                Some(Annotation::Item(item_type, indent_level))
            }
            MarkdownNode::Block(BlockNode::Rule) => Some(Annotation::Rule),
            _ => None,
        }
    }

    pub fn intersects_selection(&self, ast: &Ast, cursor: Cursor) -> bool {
        if let Some(&ast_node_idx) = self.ancestors.last() {
            ast.nodes[ast_node_idx]
                .range
                .intersects_allow_empty(&cursor.selection)
        } else {
            false
        }
    }
}

impl RangeExt<DocCharOffset> for AstTextRange {
    /// returns whether the range includes the value
    fn contains(&self, value: DocCharOffset, start_inclusive: bool, end_inclusive: bool) -> bool {
        self.range.contains(value, start_inclusive, end_inclusive)
    }

    /// returns whether the range intersects another range
    fn intersects(
        &self, other: &(DocCharOffset, DocCharOffset), allow_empty_intersection: bool,
    ) -> bool {
        self.range.intersects(other, allow_empty_intersection)
    }

    fn start(&self) -> DocCharOffset {
        self.range.start()
    }

    fn end(&self) -> DocCharOffset {
        self.range.end()
    }

    fn len(&self) -> RelCharOffset {
        self.range.len()
    }

    fn is_empty(&self) -> bool {
        self.range.is_empty()
    }
}

pub struct AstTextRangeIter<'ast> {
    /// AST being iterated
    ast: &'ast Ast,

    /// Element last emitted by the iterator
    maybe_current_range: Option<AstTextRange>,
}

impl<'ast> Iterator for AstTextRangeIter<'ast> {
    type Item = AstTextRange;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(current_range) = &self.maybe_current_range {
            // find the next nonempty range
            let mut current_range = current_range.clone();
            let next_range = loop {
                let current_idx = current_range.ancestors.last().copied().unwrap_or_default();
                let current = &self.ast.nodes[current_idx];

                // where were we in the current node?
                current_range = match current_range.range_type {
                    AstTextRangeType::Head => {
                        // head -> advance to own text

                        // current range should have ended at start of current node's text range
                        #[cfg(debug)]
                        assert_eq!(current_range.range.1, current.text_range.0);

                        AstTextRange {
                            range_type: AstTextRangeType::Text,
                            range: (
                                current_range.range.1,
                                if current.children.is_empty() {
                                    current.text_range.1
                                } else {
                                    let first_child = &self.ast.nodes[current.children[0]];
                                    first_child.range.0
                                },
                            ),
                            ancestors: current_range.ancestors.clone(),
                        }
                    }
                    AstTextRangeType::Text => {
                        // text -> advance to next child head or advance to own tail
                        let maybe_next_child_idx = current.children.iter().find(|&&child_idx| {
                            // child of the current node starting at end of current range
                            self.ast.nodes[child_idx].range.0 == current_range.range.1
                        });

                        if let Some(&next_child_idx) = maybe_next_child_idx {
                            // next child's head
                            let next_child = &self.ast.nodes[next_child_idx];
                            let ancestors = {
                                let mut ancestors = current_range.ancestors.clone();
                                ancestors.push(next_child_idx);
                                ancestors
                            };

                            AstTextRange {
                                range_type: AstTextRangeType::Head,
                                range: (current_range.range.1, next_child.text_range.0),
                                ancestors,
                            }
                        } else {
                            // own tail

                            // current range should have ended at end of current node's text range
                            #[cfg(debug)]
                            assert_eq!(current_range.range.1, current.text_range.1);

                            AstTextRange {
                                range_type: AstTextRangeType::Tail,
                                range: (current_range.range.1, current.range.1),
                                ancestors: current_range.ancestors.clone(),
                            }
                        }
                    }
                    AstTextRangeType::Tail => {
                        // current range should have ended at end of current node's range
                        #[cfg(debug)]
                        assert_eq!(current_range.range.1, current.range.1);

                        // tail -> advance to parent text
                        // find next child of parent
                        let ancestors = {
                            let mut ancestors = current_range.ancestors.clone();
                            if ancestors.pop().is_none() {
                                break None;
                            };
                            ancestors
                        };
                        let parent_idx = ancestors.last().copied().unwrap_or_default();
                        let parent = &self.ast.nodes[parent_idx];
                        let maybe_next_child_idx = parent.children.iter().find(|&&child_idx| {
                            // first child of the parent node starting after end of current range
                            self.ast.nodes[child_idx].range.0 >= current_range.range.1
                        });

                        if let Some(&next_child_idx) = maybe_next_child_idx {
                            // range in parent node from end of current range to beginning of next child's range
                            let next_child = &self.ast.nodes[next_child_idx];

                            AstTextRange {
                                range_type: AstTextRangeType::Text,
                                range: (current_range.range.1, next_child.range.0),
                                ancestors,
                            }
                        } else {
                            // range in parent node from end of current range to end of parent node text

                            AstTextRange {
                                range_type: AstTextRangeType::Text,
                                range: (current_range.range.1, parent.text_range.1),
                                ancestors,
                            }
                        }
                    }
                };

                if !current_range.range.is_empty() {
                    break Some(current_range);
                }
            };

            self.maybe_current_range = next_range.clone();
            next_range
        } else {
            None
        }
    }
}

impl Editor {
    pub fn print_ast(&self) {
        println!("ast:");
        self.ast.print(&self.buffer.current);
    }
}

#[cfg(test)]
mod test {
    use crate::tab::markdown_editor::test_input;

    #[test]
    fn test_markdown_all_no_inverted_ranges() {
        for test_markdown in test_input::TEST_MARKDOWN_ALL {
            let buffer = test_markdown.into();
            let ast = super::calc(&buffer);
            for text_range in ast.iter_text_ranges() {
                assert!(text_range.range.0 <= text_range.range.1);
            }
        }
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
