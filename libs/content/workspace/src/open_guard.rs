//! Refuses documents that would hang the app.
//!
//! Two layers: [`text_refusal`] turns away text the editor can't lay out at an
//! interactive frame rate, and [`OpenGuard`] marks each open on disk so a
//! launch after a hang doesn't reopen the file that caused it.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use chrono::{DateTime, Local, TimeDelta};
use egui::text::LayoutJob;
use egui::{Align, Layout, Rect, Ui, pos2, vec2};
use lb_rs::Uuid;
use lb_rs::model::usage::bytes_to_human;
use tracing::warn;
use web_time::{Duration, Instant};

use crate::style::{Button, Space, Spacer, ThemeExt as _, TypeRole, control_height, phosphor};
use crate::tab::{ContentState, Tab};

/// Editor frame time grows ~60ms per MB of ordinary text.
const MAX_TEXT_BYTES: usize = 1_000_000;

/// An on-screen line costs ~2ms per 1,000 characters each frame when it has
/// no spaces to wrap at. A markdown paragraph lays out as one line, and its
/// cost grows faster than its length (~0.5s a frame at 100,000).
const MAX_LINE_CHARS: usize = 10_000;

/// Typing in a fenced code block costs ~180ms a keystroke at this size and
/// grows faster than its length.
const MAX_CODE_BLOCK_CHARS: usize = 50_000;

/// How long the guard watches content that never settles: a worker hung
/// off the UI thread hangs nothing.
const WATCH_BOUND: Duration = Duration::from_secs(10);

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

/// The document the previous launch never finished opening.
#[derive(Debug, PartialEq)]
pub struct Interrupted {
    pub id: Uuid,
    /// A panic was logged after the open began; otherwise the app hung or was killed.
    pub crashed: bool,
}

impl Interrupted {
    pub fn refusal(&self) -> Refusal {
        let (title, detail) = if self.crashed {
            ("Lockbook crashed", "Last time we tried opening this file, it didn't go so well.")
        } else {
            (
                "Lockbook stopped responding",
                "Last time we started opening this file, we never finished.",
            )
        };
        Refusal { title, detail: detail.into() }
    }
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

/// Holds the documents being opened in a marker file for the duration of
/// each open, so the marker outlives a hang the user force-quits, and
/// refuses this session what the last one never finished opening. Shared by
/// every open: a tab, an embed, a headless render.
pub struct OpenGuard {
    marker: Option<PathBuf>,
    state: Mutex<GuardState>,
}

#[derive(Default)]
struct GuardState {
    /// Documents being opened, each once per open in progress.
    opening: Vec<Uuid>,
    /// What the previous launch never finished opening.
    interrupted: Vec<Interrupted>,
    /// Refused documents the user opened anyway this session.
    opened_anyway: Vec<Uuid>,
}

impl OpenGuard {
    pub fn new(writeable_dir: &Path, enabled: bool, last_panic: Option<DateTime<Local>>) -> Self {
        let state = Mutex::new(GuardState::default());
        if !enabled {
            return Self { marker: None, state };
        }
        let marker = writeable_dir.join("ws_opening");
        let started = fs::metadata(&marker)
            .and_then(|m| m.modified())
            .ok()
            .map(DateTime::<Local>::from);
        // panic times are truncated to the second
        let crashed = last_panic
            .zip(started)
            .is_some_and(|(panic, started)| panic + TimeDelta::seconds(1) >= started);
        let mut interrupted: Vec<Interrupted> = vec![];
        for line in fs::read_to_string(&marker).unwrap_or_default().lines() {
            let Ok(id) = Uuid::parse_str(line.trim()) else { continue };
            if interrupted.iter().all(|i| i.id != id) {
                interrupted.push(Interrupted { id, crashed });
            }
        }
        if !interrupted.is_empty() {
            warn!(?interrupted, "previous launch never finished opening documents");
            let _ = fs::remove_file(&marker);
        }
        state.lock().unwrap().interrupted = interrupted;
        Self { marker: Some(marker), state }
    }

    /// What the previous launch never finished opening.
    pub fn interrupted(&self) -> Vec<Uuid> {
        let state = self.state.lock().unwrap();
        state.interrupted.iter().map(|i| i.id).collect()
    }

    /// Why `id` shouldn't open before its bytes are read, if it shouldn't.
    pub fn refusal_before_load(&self, id: Uuid) -> Option<Refusal> {
        let state = self.state.lock().unwrap();
        let interrupted = state.interrupted.iter().find(|i| i.id == id)?;
        Some(interrupted.refusal())
    }

    /// Why `id` shouldn't open with `bytes`, if it shouldn't. `text` says
    /// the bytes are bound for the editor, and whether as markdown.
    pub fn refusal(&self, id: Uuid, bytes: &[u8], text: Option<bool>) -> Option<Refusal> {
        if self.state.lock().unwrap().opened_anyway.contains(&id) {
            return None;
        }
        self.refusal_before_load(id)
            .or_else(|| text.and_then(|markdown| text_refusal(bytes, markdown)))
    }

    /// The user chose to open a refused document; it opens from now on.
    pub fn open_anyway(&self, id: Uuid) {
        let mut state = self.state.lock().unwrap();
        state.opened_anyway.push(id);
        state.interrupted.retain(|i| i.id != id);
    }

    pub fn enter(&self, id: Uuid) {
        let mut state = self.state.lock().unwrap();
        state.opening.push(id);
        self.write(&state.opening);
    }

    pub fn exit(&self, id: Uuid) {
        let mut state = self.state.lock().unwrap();
        if let Some(i) = state.opening.iter().position(|o| *o == id) {
            state.opening.swap_remove(i);
        }
        self.write(&state.opening);
    }

    fn write(&self, opening: &[Uuid]) {
        let Some(marker) = &self.marker else { return };
        if opening.is_empty() {
            let _ = fs::remove_file(marker);
        } else {
            let ids: Vec<String> = opening.iter().map(Uuid::to_string).collect();
            let _ = fs::write(marker, ids.join("\n"));
        }
    }

    /// Shows `tab`, holding the marker through each frame of its content
    /// until it settles. The marker exists only mid-frame, so a suspended
    /// app the system kills leaves none.
    pub fn show<R>(&self, tab: &mut Tab, show: impl FnOnce(&mut Tab) -> R) -> R {
        let open = matches!(tab.content, ContentState::Open(_));
        if open && tab.first_draw {
            tab.watch = Some(Instant::now());
        }
        let Some(id) = tab.id().filter(|_| open && tab.watch.is_some()) else {
            return show(tab);
        };
        self.enter(id);
        let result = show(tab);
        self.exit(id);
        if tab.settled() || tab.watch.is_some_and(|at| at.elapsed() > WATCH_BOUND) {
            tab.watch = None;
        }
        result
    }
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

    #[test]
    fn marker_survives_a_hang() {
        let dir = std::env::temp_dir().join(Uuid::new_v4().to_string());
        fs::create_dir_all(&dir).unwrap();
        let launch = |last_panic| {
            let guard = OpenGuard::new(&dir, true, last_panic);
            let state = guard.state.into_inner().unwrap();
            state.interrupted
        };
        let guard = OpenGuard::new(&dir, true, None);
        assert!(guard.interrupted().is_empty());

        let (id, embed) = (Uuid::new_v4(), Uuid::new_v4());
        guard.enter(id);
        guard.exit(id);
        assert_eq!(launch(None), vec![]);

        // force-quit mid-open of a note and the document it embeds, after an older panic
        let before = Local::now() - TimeDelta::seconds(60);
        guard.enter(id);
        guard.enter(embed);
        let crashed = false;
        assert_eq!(
            launch(Some(before)),
            vec![Interrupted { id, crashed }, Interrupted { id: embed, crashed }]
        );
        assert_eq!(launch(None), vec![]);

        // panic mid-open, logged within the same second; the embed's open finished
        let guard = OpenGuard::new(&dir, true, None);
        guard.enter(id);
        guard.enter(embed);
        guard.exit(embed);
        let panic = Local::now() - TimeDelta::milliseconds(900);
        assert_eq!(launch(Some(panic)), vec![Interrupted { id, crashed: true }]);
        fs::remove_dir_all(&dir).unwrap();
    }
}
