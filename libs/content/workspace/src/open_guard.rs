//! Refuses documents that would hang the app: [`text_refusal`] turns away
//! text the editor can't lay out at an interactive frame rate.

use egui::text::LayoutJob;
use egui::{Align, Layout, Rect, Ui, pos2, vec2};
use lb_rs::model::usage::bytes_to_human;

use crate::style::{Button, Space, Spacer, ThemeExt as _, TypeRole, control_height, phosphor};

/// Editor frame time grows ~60ms per MB of ordinary text.
const MAX_TEXT_BYTES: usize = 1_000_000;

/// An on-screen line costs ~2ms per 1,000 characters each frame when it has
/// no spaces to wrap at. A markdown paragraph lays out as one line, and its
/// cost grows faster than its length (~0.5s a frame at 100,000).
const MAX_LINE_CHARS: usize = 10_000;

/// Typing in a fenced code block costs ~180ms a keystroke at this size and
/// grows faster than its length.
const MAX_CODE_BLOCK_CHARS: usize = 50_000;

#[derive(Debug)]
pub struct Refusal {
    pub title: &'static str,
    pub detail: String,
}

/// Why text bound for the editor shouldn't open, if it shouldn't. Markdown
/// structure is approximated line by line; nothing is parsed.
pub fn text_refusal(bytes: &[u8], markdown: bool) -> Option<Refusal> {
    if bytes.len() > MAX_TEXT_BYTES {
        return Some(Refusal {
            title: "Too large to open",
            detail: format!(
                "This file is {}. Opening text files over {} is not supported.",
                bytes_to_human(bytes.len() as u64),
                bytes_to_human(MAX_TEXT_BYTES as u64),
            ),
        });
    }

    let (mut longest_line, mut longest_para, mut longest_code) = (0, 0, 0);
    // open fence: (marker byte, marker length, characters inside)
    let (mut para, mut code) = (0, None);
    for line in bytes.split(|&b| b == b'\n') {
        let chars = line.iter().filter(|&&b| b & 0xC0 != 0x80).count();
        longest_line = longest_line.max(chars);
        if !markdown {
            continue;
        }
        let trimmed = line.trim_ascii_start();
        if let Some((marker, n, len)) = &mut code {
            if fence(trimmed).is_some_and(|(m, k)| m == *marker && k >= *n) {
                code = None;
            } else {
                *len += chars;
                longest_code = longest_code.max(*len);
            }
        } else if let Some((marker, n)) = fence(trimmed) {
            (code, para) = (Some((marker, n, 0)), 0);
        } else if trimmed.iter().all(|b| b" >\t\r".contains(b)) {
            para = 0;
        } else if starts_block(line, trimmed) {
            para = chars;
        } else {
            para += chars;
        }
        longest_para = longest_para.max(para);
    }

    let refusal = |title, what: &str, len, max| {
        Some(Refusal {
            title,
            detail: format!(
                "One {what} is {} characters long. Opening files with {what}s over {} characters is not supported.",
                thousands(len),
                thousands(max),
            ),
        })
    };
    if longest_line > MAX_LINE_CHARS {
        refusal("Line too long to open", "line", longest_line, MAX_LINE_CHARS)
    } else if longest_para > MAX_LINE_CHARS {
        refusal("Paragraph too long to open", "paragraph", longest_para, MAX_LINE_CHARS)
    } else if longest_code > MAX_CODE_BLOCK_CHARS {
        refusal("Code block too long to open", "code block", longest_code, MAX_CODE_BLOCK_CHARS)
    } else {
        None
    }
}

/// A code fence's marker byte and length.
fn fence(trimmed: &[u8]) -> Option<(u8, usize)> {
    let marker = *trimmed.first().filter(|&&b| b == b'`' || b == b'~')?;
    let n = trimmed.iter().take_while(|&&b| b == marker).count();
    (n >= 3).then_some((marker, n))
}

/// Lines laid out apart from the paragraph above them (quote markers continue one).
fn starts_block(line: &[u8], trimmed: &[u8]) -> bool {
    let digits = trimmed.iter().take_while(|b| b.is_ascii_digit()).count();
    line.starts_with(b"    ")
        || line.starts_with(b"\t")
        || trimmed.contains(&b'|')
        || matches!(trimmed, [b'#' | b'<', ..] | [b'-' | b'*' | b'+', b' ' | b'\t', ..])
        || digits > 0 && matches!(trimmed.get(digits), Some(b'.' | b')'))
}

fn thousands(n: usize) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// Centered explanation with an "Open anyway" button; returns whether it was clicked.
pub fn show_refusal(ui: &mut Ui, refusal: &Refusal) -> bool {
    let t = ui.ctx().get_lb_theme();
    let rect = ui.available_rect_before_wrap();
    let wrap_w = (rect.width() - Space::Lg.pts() * 2.0).clamp(40.0, 420.0);
    let centered = |text: &str, font, color| {
        let mut job = LayoutJob::simple(text.into(), font, color, wrap_w);
        job.halign = Align::Center;
        ui.painter().layout_job(job)
    };
    let icon =
        centered(phosphor::WARNING, crate::style::phosphor_ui_font_id(), t.neutral_fg_secondary());
    let title = centered(refusal.title, TypeRole::Heading.font_id(), t.neutral_fg());
    let detail = centered(&refusal.detail, TypeRole::Body.font_id(), t.neutral_fg_secondary());
    let content_h = icon.size().y
        + Space::Sm.pts()
        + title.size().y
        + Space::Xxs.pts()
        + detail.size().y
        + Space::Md.pts()
        + control_height();
    let y = (rect.center().y - content_h / 2.0).max(rect.top());
    let block = Rect::from_min_size(
        pos2(rect.left(), y),
        vec2(rect.width(), content_h.min(rect.height()).max(1.0)),
    );

    let (clicked, _) = crate::style::place_at(ui, block, Layout::top_down(Align::Center), |ui| {
        for (galley, space) in [(icon, Space::Sm), (title, Space::Xxs), (detail, Space::Md)] {
            let (r, _) = ui.allocate_exact_size(galley.size(), egui::Sense::hover());
            ui.painter().galley(r.center_top(), galley, t.neutral_fg());
            ui.add(Spacer::new(space));
        }
        Button::secondary(&t, "Open anyway").show(ui).clicked()
    });
    crate::style::claim(ui, rect);
    clicked
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refuses_large_text() {
        assert!(text_refusal(&vec![b'a'; MAX_TEXT_BYTES + 1], false).is_some());
        assert!(text_refusal("ab\n".repeat(MAX_TEXT_BYTES / 3).as_bytes(), false).is_none());
    }

    #[test]
    fn refuses_long_lines_by_char() {
        let line = "é".repeat(MAX_LINE_CHARS);
        assert!(text_refusal(format!("{line}\n{line}").as_bytes(), false).is_none());
        assert!(text_refusal(format!("{line}é").as_bytes(), false).is_some());
    }

    #[test]
    fn refuses_long_markdown_blocks() {
        let refused = |md: &str| text_refusal(md.as_bytes(), true).map(|r| r.title);
        let lines = |prefix: &str, n| {
            (0..n)
                .map(|i| format!("{prefix}line {i:04}\n"))
                .collect::<String>()
        };

        // ~10 chars a line: 1,500 lines joined into one paragraph, but not as list items or across blank lines
        assert_eq!(refused(&lines("", 1500)), Some("Paragraph too long to open"));
        assert_eq!(refused(&lines("> ", 1500)), Some("Paragraph too long to open"));
        assert_eq!(refused(&lines("- ", 1500)), None);
        assert_eq!(refused(&lines("", 1500).replace("0\n", "0\n\n")), None);
        assert_eq!(refused(&lines("> ", 1500).replace("0\n", "0\n>\n")), None);
        assert_eq!(refused(&lines("a | ", 1500)), None);
        assert_eq!(text_refusal(lines("", 1500).as_bytes(), false).map(|r| r.title), None);

        let code = |n| format!("```\n{}```\n", lines("", n));
        assert_eq!(refused(&code(1500)), None);
        assert_eq!(refused(&code(6000)), Some("Code block too long to open"));
        // a shorter fence inside a longer one doesn't close it
        let nested = format!("````\n```\n{}````\n", lines("", 1500));
        assert_eq!(refused(&nested), None);
    }
}
