use chrono::DateTime;
use lb_rs::model::text::offset_types::Grapheme;

use super::super::widget::inline::timestamp::Clock;
use super::super::widget::utils::wrap_layout::FragmentContent;
use super::render_props::{render_frame, test_renderer};

/// The text painted for `md`, with the caret at `caret` if any.
fn painted(md: &str, caret: Option<usize>) -> String {
    let mut r = test_renderer(md);
    r.clock = Clock::Fixed(DateTime::parse_from_rfc3339("2026-11-15T23:00:00-05:00").unwrap());
    let selection = caret.map(|c| (Grapheme(c), Grapheme(c)));
    r.reveal_selection = selection;
    render_frame(&mut r, 2000.0, selection, |r| {
        r.fragments
            .iter()
            .filter_map(|f| match &f.content {
                FragmentContent::Glyphs { buffer, .. } => Some(
                    buffer
                        .read()
                        .unwrap()
                        .lines
                        .iter()
                        .map(|l| l.text().to_string())
                        .collect::<String>(),
                ),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join(" ")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    })
}

#[test]
fn timestamp_reads_as_a_time() {
    let md = "Built 2026-11-15T22:34:00-05:00 today.\n";
    let shown = painted(md, None);
    assert!(shown.contains("10:34 PM"), "{shown}");
    assert!(!shown.contains("2026"), "{shown}");
}

#[test]
fn timestamp_in_a_table_cell_reads_as_a_time() {
    let md = "| When | Task |\n|---|---|\n| 2026-11-16T03:40:10Z | plan |\n";
    let shown = painted(md, None);
    assert!(shown.contains("10:40 PM"), "{shown}");
    assert!(!shown.contains("2026"), "{shown}");
}

#[test]
fn caret_in_a_timestamp_shows_its_source() {
    let md = "Built 2026-11-15T22:34:00-05:00 today.\n";
    for caret in [6, 20, 31] {
        let shown = painted(md, Some(caret));
        assert!(shown.contains("2026-11-15T22:34:00-05:00"), "caret {caret}: {shown}");
    }
    let shown = painted(md, Some(2));
    assert!(shown.contains("10:34 PM"), "{shown}");
}

#[test]
fn timestamp_stays_raw_outside_prose() {
    let stamp = "2026-11-15T22:34:00-05:00";
    for md in [
        format!("`{stamp}`\n"),
        format!("```\n{stamp}\n```\n"),
        format!("[{stamp}](https://x.test/{stamp})\n"),
        format!("<https://x.test/{stamp}>\n"),
        format!("---\nat: {stamp}\n---\n\nbody\n"),
        "2026-11-15T22:34:00 and 2026-13-40T22:34:00Z\n".into(),
    ] {
        let shown = painted(&md, None);
        assert!(!shown.contains("10:34 PM"), "{md:?}: {shown}");
    }
}
