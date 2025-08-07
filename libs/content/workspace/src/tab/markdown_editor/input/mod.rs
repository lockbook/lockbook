pub mod advance;
pub mod canonical;
pub mod cursor;
pub mod events;
pub mod mutation;

use egui::Pos2;

use crate::tab::markdown_editor;
use lb_rs::model::text::offset_types::DocCharOffset;
use markdown_editor::style::MarkdownNode;

/*
 * This module processes input events, with the following major concerns:
 * * Plumbing: combining programmatic and UI input, delegating to appropriate handlers
 * * Enrichment: did the user click on a link, or select a word, or drag a selection?
 * * Buffer manipulation: text replacements & cursor movements, operational transformation, merging concurrent edits
 */

/// text location
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Location {
    CurrentCursor,
    DocCharOffset(DocCharOffset),
    Pos(Pos2),
}

/// text unit that has a start and end location
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Bound {
    Char,
    Word,
    Line,
    Paragraph,
    Doc,
}

/// text unit you can increment or decrement a location by
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Increment {
    Line,
}

/// text location relative to some absolute text location
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Offset {
    /// text location at a bound; if you're already there, this leaves you there
    /// (e.g. cmd+left/right)
    To(Bound),

    /// text location at the next bound; if you're already there, this goes to
    /// the next one (e.g. option+left/right)
    Next(Bound),

    /// text location some increment away; if you're in the middle of one of
    /// these, this goes somewhere in the middle of the next one (e.g. up/down)
    By(Increment),
}

/// text region specified in some manner
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Region {
    /// 0-length region starting and ending at location
    Location(Location),

    /// Text from secondary cursor to location. Preserves selection.
    ToLocation(Location),

    /// Text from one location to another
    BetweenLocations { start: Location, end: Location },

    /// Currently selected text
    Selection,

    /// Currently selected text, or if the selection is empty, text from the
    /// primary cursor to one char/line before/after or to start/end of
    /// word/line/doc
    SelectionOrOffset { offset: Offset, backwards: bool },

    /// Text from primary cursor to one char/line before/after or to start/end
    /// of word/line/paragraph/doc. In some situations this instead represents
    /// the start of selection (if `backwards`) or end of selection, based on
    /// what feels intuitive when using arrow keys to navigate a document.
    ToOffset { offset: Offset, backwards: bool, extend_selection: bool },

    /// Current word/line/paragraph/doc, preferring previous word if `backwards`
    Bound { bound: Bound, backwards: bool },

    /// Word/line/paragraph/doc at a location, preferring previous word if `backwards`
    BoundAt { bound: Bound, location: Location, backwards: bool },
}

/// Standardized edits to any editor state e.g. buffer, clipboard, debug state.
/// Interpretation may depend on render state e.g. galley positions, line wrap.
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    Select { region: Region },
    Replace { region: Region, text: String, advance_cursor: bool }, // replace region with text and optionally advance cursor to end of new text
    ToggleStyle { region: Region, style: MarkdownNode }, // supports toolbar and inline tyle keyboard shortcuts
    Newline { shift: bool }, // distinct from replace because it triggers auto-bullet, etc
    Delete { region: Region }, // distinct from replace because it triggers numbered list renumber, etc
    Indent { deindent: bool }, // distinct from replace because it's a no-op for first list item, etc
    Find { term: String, backwards: bool },
    Undo,
    Redo,
    Cut,
    Copy,
    ToggleDebug,
    IncrementBaseFontSize,
    DecrementBaseFontSize,
}

impl From<(DocCharOffset, DocCharOffset)> for Region {
    fn from((start, end): (DocCharOffset, DocCharOffset)) -> Self {
        Region::BetweenLocations {
            start: Location::DocCharOffset(start),
            end: Location::DocCharOffset(end),
        }
    }
}
