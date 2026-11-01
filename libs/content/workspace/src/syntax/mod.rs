//! Code coloring with tree-sitter: languages by fence token and file
//! extension, and spans of [`Role`]s over a unit of code (one code block or
//! one code file). The web build has no grammars, so it knows no language.

use std::ops::Range;
use std::sync::Arc;

#[cfg(not(target_family = "wasm"))]
mod highlight;
#[cfg(not(target_family = "wasm"))]
mod languages;

/// What a span of code is. Markup adds the roles drawn bold or italic.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Role {
    Text,
    Comment,
    Class,
    Function,
    Keyword,
    String,
    Number,
    Heading,
    Emphasis,
    Strong,
}

/// Byte ranges into a unit of code, sorted and disjoint. Bytes outside
/// every span are [`Role::Text`].
pub type Spans = Vec<(Role, Range<usize>)>;

/// A named item of code, like a function or a type, `depth` items deep:
/// where it starts and where its name is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Definition {
    pub start: usize,
    pub name: Range<usize>,
    pub depth: usize,
}

/// A language we have a grammar for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Lang(usize);

#[cfg(not(target_family = "wasm"))]
impl Lang {
    /// The language a fence's info string names by its first word, which
    /// may be a name, an alias, or a file extension; `rust,no_run` names Rust.
    pub fn from_info(info: &str) -> Option<Self> {
        let token = info.split([' ', '\t', ',']).next()?;
        languages::find_token(token).map(Lang)
    }

    /// The language of a file with this extension.
    pub fn from_extension(ext: &str) -> Option<Self> {
        languages::find_extension(ext).map(Lang)
    }

    /// `text` parsed as this language; none if its grammar won't load.
    pub fn highlights(self, text: String) -> Option<Highlights> {
        highlight::Highlights::new(self.0, text).map(Highlights)
    }

    /// Whether [`Highlights::definitions`] finds anything in this language.
    pub fn has_definitions(self) -> bool {
        languages::has_definitions(self.0)
    }
}

/// A version of a unit of code, parsed, with the spans asked of it.
#[cfg(not(target_family = "wasm"))]
pub struct Highlights(highlight::Highlights);

#[cfg(not(target_family = "wasm"))]
impl Highlights {
    /// `text` parsed again from this version, for the next version of the
    /// same unit.
    pub fn edited(&self, text: String) -> Option<Self> {
        self.0.edited(text).map(Self)
    }

    /// The spans within `range` of the text, clipped to it.
    pub fn spans(&self, range: Range<usize>) -> Arc<Spans> {
        self.0.spans(range)
    }

    /// The functions, types, and the like, in order.
    pub fn definitions(&self) -> Vec<Definition> {
        languages::definitions(self.0.lang(), self.0.tree(), self.0.text())
    }

    /// The smallest syntax node larger than `range` and covering it.
    pub fn enclosing(&self, range: Range<usize>) -> Option<Range<usize>> {
        languages::enclosing(self.0.tree(), range)
    }
}

/// How many bytes two texts agree on at their start, plus at their end.
#[cfg(not(target_family = "wasm"))]
pub fn agreeing(a: &str, b: &str) -> usize {
    let (start, end) = highlight::agreeing(a, b);
    start + end
}

#[cfg(target_family = "wasm")]
impl Lang {
    pub fn from_info(_info: &str) -> Option<Self> {
        None
    }

    pub fn from_extension(_ext: &str) -> Option<Self> {
        None
    }

    pub fn highlights(self, _text: String) -> Option<Highlights> {
        unreachable!("no language exists on the web")
    }

    pub fn has_definitions(self) -> bool {
        unreachable!("no language exists on the web")
    }
}

/// No code is parsed on the web.
#[cfg(target_family = "wasm")]
pub enum Highlights {}

#[cfg(target_family = "wasm")]
impl Highlights {
    pub fn edited(&self, _text: String) -> Option<Self> {
        match *self {}
    }

    pub fn spans(&self, _range: Range<usize>) -> Arc<Spans> {
        match *self {}
    }

    pub fn definitions(&self) -> Vec<Definition> {
        match *self {}
    }

    pub fn enclosing(&self, _range: Range<usize>) -> Option<Range<usize>> {
        match *self {}
    }
}

#[cfg(target_family = "wasm")]
pub fn agreeing(_a: &str, _b: &str) -> usize {
    0
}

/// Language names a fence's info could be completing `query` toward, best
/// first.
#[cfg(not(target_family = "wasm"))]
pub fn complete(query: &str) -> Vec<&'static str> {
    languages::complete(query)
}

#[cfg(target_family = "wasm")]
pub fn complete(_query: &str) -> Vec<&'static str> {
    Vec::new()
}

/// The roles over `range`, a slice of the unit `spans` cover, in order and
/// covering all of `range`. Ranges are relative to the unit.
pub fn roles_in(spans: &[(Role, Range<usize>)], range: Range<usize>) -> Vec<(Role, Range<usize>)> {
    let mut result = Vec::new();
    let mut at = range.start;
    let first = spans.partition_point(|(_, span)| span.end <= range.start);
    for (role, span) in &spans[first..] {
        if span.start >= range.end {
            break;
        }
        let start = span.start.max(range.start);
        let end = span.end.min(range.end);
        if start > at {
            result.push((Role::Text, at..start));
        }
        result.push((*role, start..end));
        at = end;
    }
    if at < range.end {
        result.push((Role::Text, at..range.end));
    }
    result
}

#[cfg(test)]
mod test {
    use super::{Role, roles_in};

    #[test]
    fn roles_cover_the_range() {
        let spans = [(Role::Keyword, 0..2), (Role::Comment, 4..10)];
        assert_eq!(
            roles_in(&spans, 1..6),
            vec![(Role::Keyword, 1..2), (Role::Text, 2..4), (Role::Comment, 4..6)]
        );
        assert_eq!(roles_in(&spans, 10..12), vec![(Role::Text, 10..12)]);
    }
}
