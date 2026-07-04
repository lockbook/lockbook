//! Rendered diffs as *segments*: each run of changed blocks becomes its own
//! independently-parsed markdown snippet (old blocks, then new blocks), so
//! the viewer stacks and tints them separately. Splicing whole blocks (by
//! the renderer's own parse) keeps every segment complete and valid — and
//! parsing segments separately means a deleted list and its replacement can
//! never merge into one loose list the way a combined document's would.

use comrak::Arena;

use crate::tab::markdown_editor::MdRender;

/// One stacked piece of a rendered diff: consecutive same-kind blocks,
/// joined as one markdown snippet.
#[derive(Clone)]
pub struct Segment {
    pub text: String,
    pub kind: SegKind,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SegKind {
    /// Unchanged surroundings (the approval card keeps one block of them;
    /// row bodies keep none).
    Context,
    Del,
    Add,
}

/// Diff `old` → `new` at block granularity into stacked segments, keeping
/// `context` unchanged blocks around each changed run. Elision is implicit —
/// dropped blocks simply don't appear.
pub fn segments(old: &str, new: &str, context: usize) -> Vec<Segment> {
    use similar::{ChangeTag, TextDiff};
    let old_blocks = blocks(old);
    let new_blocks = blocks(new);
    let old_refs: Vec<&str> = old_blocks.iter().map(String::as_str).collect();
    let new_refs: Vec<&str> = new_blocks.iter().map(String::as_str).collect();
    let diff = TextDiff::from_slices(&old_refs, &new_refs);
    let tagged: Vec<(&str, ChangeTag)> = diff
        .iter_all_changes()
        .map(|c| (c.value(), c.tag()))
        .collect();

    let keep = |i: usize| {
        let lo = i.saturating_sub(context);
        let hi = (i + context).min(tagged.len().saturating_sub(1));
        tagged[lo..=hi].iter().any(|(_, t)| *t != ChangeTag::Equal)
    };

    let mut out: Vec<Segment> = Vec::new();
    for (i, (block, tag)) in tagged.iter().enumerate() {
        if !keep(i) {
            continue;
        }
        let kind = match tag {
            ChangeTag::Insert => SegKind::Add,
            ChangeTag::Delete => SegKind::Del,
            ChangeTag::Equal => SegKind::Context,
        };
        match out.last_mut() {
            // Consecutive same-kind kept blocks join into one snippet.
            Some(last) if last.kind == kind && keep(i.saturating_sub(1)) && i > 0 => {
                last.text.push_str("\n\n");
                last.text.push_str(block);
            }
            _ => out.push(Segment { text: (*block).to_string(), kind }),
        }
    }
    out
}

/// Top-level markdown blocks of `text`, segmented by the renderer's own
/// parse (same comrak options), so the diff's units are exactly the blocks
/// the segments will render.
fn blocks(text: &str) -> Vec<String> {
    let arena = Arena::new();
    // The renderer parses with a trailing newline; match it.
    let source = format!("{text}\n");
    let root = comrak::parse_document(&arena, &source, &MdRender::comrak_options());
    let lines: Vec<&str> = source.lines().collect();
    root.children()
        .filter_map(|node| {
            let sp = node.data.borrow().sourcepos;
            let start = sp.start.line.checked_sub(1)?;
            let end = sp.end.line.min(lines.len());
            let block = lines.get(start..end)?.join("\n");
            (!block.trim().is_empty()).then_some(block)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat(segs: &[Segment]) -> Vec<(&str, SegKind)> {
        segs.iter().map(|s| (s.text.as_str(), s.kind)).collect()
    }

    /// Zero context (row bodies): nothing but the changed blocks, old then
    /// new, each its own segment.
    #[test]
    fn zero_context_is_just_the_change() {
        let old = "# Title\n\nkeep this.\n\ndrop this one.\n";
        let new = "# Title\n\nkeep this.\n\na new one.\n\nand a tail.\n";
        let segs = segments(old, new, 0);
        assert_eq!(
            flat(&segs),
            [("drop this one.", SegKind::Del), ("a new one.\n\nand a tail.", SegKind::Add)]
        );
    }

    /// One block of context (the approval card): the nearest unchanged
    /// blocks bracket the change; farther blocks are simply absent.
    #[test]
    fn card_context_brackets_the_change() {
        let old = "# Title\n\nkeep this.\n\ndrop this one.\n";
        let new = "# Title\n\nkeep this.\n\na new one.\n";
        let segs = segments(old, new, 1);
        assert_eq!(
            flat(&segs),
            [
                ("keep this.", SegKind::Context),
                ("drop this one.", SegKind::Del),
                ("a new one.", SegKind::Add),
            ]
        );
    }

    /// A replaced list arrives as two separate segments — parsed apart, they
    /// can't merge into one loose list.
    #[test]
    fn lists_stay_separate_segments() {
        let old = "### calls\n\n- [ ] dad\n- [ ] mom\n";
        let new = "### calls\n\n- [x] dad\n- [ ] mom\n";
        let segs = segments(old, new, 0);
        assert_eq!(
            flat(&segs),
            [("- [ ] dad\n- [ ] mom", SegKind::Del), ("- [x] dad\n- [ ] mom", SegKind::Add)]
        );
    }

    /// Identical documents produce no segments.
    #[test]
    fn equal_docs_are_empty() {
        assert!(segments("a\n\nb\n", "a\n\nb\n", 1).is_empty());
    }

    /// Distant changes produce independent runs — no marker blocks between,
    /// elision is the absence itself.
    #[test]
    fn distant_changes_are_separate_runs() {
        let old = "a\n\nb\n\nc\n\nd\n\ne\n";
        let new = "A\n\nb\n\nc\n\nd\n\nE\n";
        let segs = segments(old, new, 0);
        assert_eq!(
            flat(&segs),
            [("a", SegKind::Del), ("A", SegKind::Add), ("e", SegKind::Del), ("E", SegKind::Add),]
        );
    }
}
