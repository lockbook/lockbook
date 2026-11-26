//! RFC 3339 timestamps in prose, shown as a readable time in the reader's
//! zone. The source shows while the selection touches one, as a shortcode's
//! does.

use std::ops::Range;

use chrono::{DateTime, Datelike as _, FixedOffset, Local, TimeZone};
use comrak::nodes::{AstNode, NodeValue};
use lb_rs::model::text::offset_types::{Byte, Grapheme, RangeExt as _};

use crate::tab::markdown_editor::MdRender;
use crate::tab::markdown_editor::widget::utils::wrap_layout::{Format, Layout};

/// What "now" and the reader's zone are, for showing a timestamp.
#[derive(Clone, Copy, Debug, Default)]
pub enum Clock {
    /// The system's clock and zone.
    #[default]
    System,
    /// A fixed instant, read in its own offset.
    Fixed(DateTime<FixedOffset>),
}

impl Clock {
    /// `at` as read now: the time today, the date and time this year, and
    /// the year too before that.
    pub fn show(&self, at: DateTime<FixedOffset>) -> String {
        match self {
            Clock::System => show(at, Local::now()),
            Clock::Fixed(now) => show(at, *now),
        }
    }
}

fn show<Tz: TimeZone>(at: DateTime<FixedOffset>, now: DateTime<Tz>) -> String
where
    Tz::Offset: std::fmt::Display,
{
    let at = at.with_timezone(&now.timezone());
    let format = if at.date_naive() == now.date_naive() {
        "%-I:%M %p"
    } else if at.year() == now.year() {
        "%b %-d, %-I:%M %p"
    } else {
        "%b %-d %Y, %-I:%M %p"
    };
    at.format(format).to_string()
}

/// Each timestamp in `text`, by its bytes: `YYYY-MM-DDTHH:MM`, optional
/// seconds and fraction, then `Z` or `±HH:MM`, standing apart from the
/// letters and digits around it.
pub fn timestamps(text: &str) -> Vec<(Range<usize>, DateTime<FixedOffset>)> {
    let bytes = text.as_bytes();
    let word = |i: usize| bytes.get(i).is_some_and(|b| b.is_ascii_alphanumeric());
    let mut found = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if i > 0 && word(i - 1) || !bytes[i].is_ascii_digit() {
            i += 1;
            continue;
        }
        match timestamp_at(&bytes[i..]) {
            Some(len) if !word(i + len) => {
                if let Some(at) = parse(&text[i..i + len]) {
                    found.push((i..i + len, at));
                    i += len;
                    continue;
                }
                i += 1;
            }
            _ => i += 1,
        }
    }
    found
}

/// The length of the timestamp `s` starts with, by its shape alone.
fn timestamp_at(s: &[u8]) -> Option<usize> {
    let digits = |at: usize, n: usize| {
        s.get(at..at + n)
            .is_some_and(|d| d.iter().all(u8::is_ascii_digit))
    };
    let is = |at: usize, b: u8| s.get(at) == Some(&b);

    // YYYY-MM-DDTHH:MM
    if !(digits(0, 4)
        && is(4, b'-')
        && digits(5, 2)
        && is(7, b'-')
        && digits(8, 2)
        && is(10, b'T')
        && digits(11, 2)
        && is(13, b':')
        && digits(14, 2))
    {
        return None;
    }
    let mut len = 16;
    if is(len, b':') && digits(len + 1, 2) {
        len += 3;
        if is(len, b'.') && digits(len + 1, 1) {
            len += 2;
            while digits(len, 1) {
                len += 1;
            }
        }
    }
    if is(len, b'Z') {
        Some(len + 1)
    } else if (is(len, b'+') || is(len, b'-'))
        && digits(len + 1, 2)
        && is(len + 3, b':')
        && digits(len + 4, 2)
    {
        Some(len + 6)
    } else {
        None
    }
}

/// The instant a timestamp of the right shape names; `None` for one that
/// names no date or time, such as month 13.
fn parse(s: &str) -> Option<DateTime<FixedOffset>> {
    // RFC 3339 requires seconds; a time without them means `:00`
    let seconds = s.as_bytes().get(16) == Some(&b':');
    if seconds {
        DateTime::parse_from_rfc3339(s).ok()
    } else {
        DateTime::parse_from_rfc3339(&format!("{}:00{}", &s[..16], &s[16..])).ok()
    }
}

impl<'ast> MdRender {
    /// Lays out `range` of text `node`, each timestamp in it shown as a
    /// readable time unless the selection reveals it. A timestamp is not
    /// prose: its parts are not words to spell check.
    pub fn layout_text_with_timestamps(
        &self, layout: &mut Layout, node: &'ast AstNode<'ast>, range: (Grapheme, Grapheme),
        format: Format,
    ) {
        let prose = self.is_prose(node);
        let mut start = range.start();
        let mut prose_start = range.start();
        if self.shows_timestamps(node) {
            let bytes = self.range_to_byte(range);
            let text = &self.buffer.current.text[bytes.0.0..bytes.1.0];
            for (found, at) in timestamps(text) {
                let Some(stamp) = self.grapheme_range(bytes.0 + found.start, bytes.0 + found.end)
                else {
                    continue;
                };
                if prose {
                    layout.push_prose((prose_start, stamp.start()));
                }
                prose_start = stamp.end();
                if self.range_revealed(stamp, true) {
                    continue;
                }
                self.layout_source(layout, node, (start, stamp.start()), format.clone(), true);
                layout.push_override(stamp, &self.clock.show(at), format.clone());
                start = stamp.end();
            }
        }
        self.layout_source(layout, node, (start, range.end()), format, true);
        if prose {
            layout.push_prose((prose_start, range.end()));
        }
    }

    /// Text in a link, an image, or a wiki link is its label or its target,
    /// which say what they say.
    fn shows_timestamps(&self, node: &'ast AstNode<'ast>) -> bool {
        node.ancestors().all(|ancestor| {
            !matches!(
                ancestor.data.borrow().value,
                NodeValue::Link(_) | NodeValue::Image(_) | NodeValue::WikiLink(_)
            )
        })
    }

    /// The graphemes of a byte range, if it starts and ends on their bounds.
    fn grapheme_range(&self, start: Byte, end: Byte) -> Option<(Grapheme, Grapheme)> {
        let segs = &self.buffer.current.segs;
        let range = (segs.byte_to_char_floor(start), segs.byte_to_char_ceil(end));
        (self.range_to_byte(range) == (start, end)).then_some(range)
    }
}

#[cfg(test)]
mod tests {
    use chrono::DateTime;

    use super::{Clock, timestamps};

    fn now() -> Clock {
        Clock::Fixed(DateTime::parse_from_rfc3339("2026-11-15T23:00:00-05:00").unwrap())
    }

    fn shown(text: &str) -> Vec<String> {
        timestamps(text)
            .into_iter()
            .map(|(_, at)| now().show(at))
            .collect()
    }

    #[test]
    fn read_in_the_readers_zone() {
        assert_eq!(shown("built 2026-11-16T03:40:10Z."), ["10:40 PM"]);
        assert_eq!(shown("2026-11-15T22:34:00-05:00"), ["10:34 PM"]);
        assert_eq!(shown("2026-11-15T22:34-05:00"), ["10:34 PM"]);
        assert_eq!(shown("2026-11-15T22:34:00.123456-05:00"), ["10:34 PM"]);
        assert_eq!(shown("2026-03-02T09:05:00-05:00"), ["Mar 2, 9:05 AM"]);
        assert_eq!(shown("2025-11-15T15:40:00-05:00"), ["Nov 15 2025, 3:40 PM"]);
        assert_eq!(shown("from 2026-11-14T15:40Z to 2026-11-15T15:40Z").len(), 2);
    }

    #[test]
    fn other_forms_are_not_timestamps() {
        for text in [
            "2026-11-15T22:34:00",
            "2026-11-15 22:34:00Z",
            "2026-11-15",
            "2026-13-40T22:34:00Z",
            "2026-11-15T25:34:00Z",
            "x2026-11-15T22:34:00Z",
            "2026-11-15T22:34:00Zx",
            "2026-11-15T22:34:00+0500",
        ] {
            assert!(timestamps(text).is_empty(), "{text}");
        }
    }
}
