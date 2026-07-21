//! Shared folder-row chrome + metrics for the file tree and move picker.
//!
//! Keeps spacing in one place so the picker can match the sidebar without
//! pulling in virtualization / sticky / rename.

use egui::{FontId, Id, Painter, Response, Sense, Ui, pos2, vec2};

use crate::theme::icons;
use crate::theme::tokens::Tokens;

// ── Metrics (file_tree + move picker) ───────────────────────────────────────

/// Uniform row height (file tree load-bearing; picker matches for cohesion).
pub const ROW_H: f32 = 34.0;
pub const INDENT_BASE: f32 = 12.0;
pub const INDENT_STEP: f32 = 16.0;
pub const ICON_NAME_GAP: f32 = 8.0;
pub const TYPE_ICON_SIZE: f32 = 16.0;
/// Glyph ~16 + gap before name.
pub const TYPE_ICON_SLOT: f32 = TYPE_ICON_SIZE + ICON_NAME_GAP;
pub const NAME_FONT: f32 = 14.0;
pub const META_ICON_SIZE: f32 = 12.0;

#[derive(Clone, Copy)]
pub struct FolderRowVisual {
    pub depth: usize,
    pub expanded: bool,
    pub selected: bool,
    /// Invalid move targets stay visible but muted; still toggle expand.
    pub enabled: bool,
    pub is_root: bool,
    pub pinned: bool,
    pub shared: bool,
    /// 1..=9 for ⌘N / Ctrl+N badge (full-screen search style).
    pub shortcut: Option<u8>,
}

/// Paint one folder row (no chevron — open/closed folder icon, whole-row click).
///
/// Same interaction model as the file tree: click selects *and* toggles expand
/// when the folder has children (caller handles both from one `clicked()`).
pub fn folder_row(
    ui: &mut Ui,
    t: &Tokens,
    id: Id,
    name: &str,
    vis: FolderRowVisual,
) -> Response {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::hover());
    // Stable id so hover/click track the folder, not layout order.
    let resp = ui.interact(rect, id, Sense::click());
    let hover = ui.ctx().animate_bool(resp.id, resp.hovered());
    let painter = ui.painter();

    // Hover even on disabled (invalid dest) so the row still feels alive;
    // selection fill only when enabled.
    if vis.selected && vis.enabled {
        painter.rect_filled(rect, 5.0, t.surface().lerp_to_gamma(t.fg(), 0.10));
    } else if hover > 0.0 {
        painter.rect_filled(rect, 5.0, t.surface().lerp_to_gamma(t.fg(), 0.05 * hover));
    }

    let ink = if vis.enabled {
        t.fg()
    } else {
        t.text_muted()
    };
    let icon_ink = if vis.enabled {
        t.accent()
    } else {
        t.text_muted()
    };
    let muted = t.text_muted();

    let cy = rect.center().y;
    let mut x = rect.left() + INDENT_BASE + vis.depth as f32 * INDENT_STEP;

    // Reserve trailing shortcut so the name doesn't run under it.
    let shortcut_w = vis.shortcut.map(|_| 28.0).unwrap_or(0.0);
    let right_limit = rect.right() - 8.0 - shortcut_w;

    // Folder open/closed is the expand cue (file tree style — no caret column).
    let icon = if vis.is_root {
        icons::FOLDER
    } else if vis.expanded {
        icons::FOLDER_OPEN
    } else {
        icons::FOLDER
    };
    let ig = painter.layout_no_wrap(icon.into(), icons::font(TYPE_ICON_SIZE), icon_ink);
    painter.galley(pos2(x, cy - ig.size().y / 2.0), ig, icon_ink);
    x += TYPE_ICON_SLOT;

    let label = if vis.is_root { "Home" } else { name };
    let name_g = painter.layout_no_wrap(label.into(), FontId::proportional(NAME_FONT), ink);
    let name_w = name_g.size().x.min((right_limit - x).max(0.0));
    painter.galley(pos2(x, cy - name_g.size().y / 2.0), name_g, ink);
    x += name_w;

    paint_trailing_marks(painter, &mut x, cy, vis.pinned, vis.shared, muted);
    if let Some(n) = vis.shortcut {
        paint_shortcut_badge(painter, rect.right() - 8.0, cy, n, muted);
    }
    resp
}

/// Trailing pin + users marks (same order/size as the file tree).
pub fn paint_trailing_marks(
    painter: &Painter, x: &mut f32, cy: f32, pinned: bool, shared: bool, ink: egui::Color32,
) {
    let font = icons::font(META_ICON_SIZE);
    if pinned {
        *x += ICON_NAME_GAP;
        let g = painter.layout_no_wrap(icons::PUSH_PIN.into(), font.clone(), ink);
        let w = g.size().x;
        painter.galley(pos2(*x, cy - g.size().y / 2.0), g, ink);
        *x += w;
    }
    if shared {
        *x += ICON_NAME_GAP;
        let g = painter.layout_no_wrap(icons::USERS.into(), font, ink);
        let w = g.size().x;
        painter.galley(pos2(*x, cy - g.size().y / 2.0), g, ink);
        *x += w;
    }
}

/// Search hit: folder icon + name + marks on line 1, muted path on line 2.
#[allow(clippy::too_many_arguments)]
pub fn search_folder_row(
    ui: &mut Ui,
    t: &Tokens,
    id: Id,
    name: &str,
    path: &str,
    selected: bool,
    enabled: bool,
    pinned: bool,
    shared: bool,
    shortcut: Option<u8>,
) -> Response {
    let h = 44.0;
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), h), Sense::hover());
    let resp = ui.interact(rect, id, Sense::click());
    let hover = ui.ctx().animate_bool(resp.id, resp.hovered());
    let painter = ui.painter();

    if selected && enabled {
        painter.rect_filled(rect, 5.0, t.surface().lerp_to_gamma(t.fg(), 0.10));
    } else if hover > 0.0 {
        painter.rect_filled(rect, 5.0, t.surface().lerp_to_gamma(t.fg(), 0.05 * hover));
    }

    let ink = if enabled {
        t.fg()
    } else {
        t.text_muted()
    };
    let muted = t.text_muted();
    let icon_ink = if enabled { t.accent() } else { muted };

    let shortcut_w = shortcut.map(|_| 28.0).unwrap_or(0.0);
    let mut x = rect.left() + INDENT_BASE;
    let name_g = painter.layout_no_wrap(name.into(), FontId::proportional(NAME_FONT), ink);
    let path_g = painter.layout_no_wrap(path.into(), FontId::proportional(11.0), muted);
    let name_h = name_g.size().y;
    let name_w = name_g.size().x;
    let stack_h = name_h + 2.0 + path_g.size().y;
    let y0 = rect.center().y - stack_h / 2.0;

    let ig = painter.layout_no_wrap(icons::FOLDER.into(), icons::font(TYPE_ICON_SIZE), icon_ink);
    let icon_y = y0 + (name_h - ig.size().y) * 0.5;
    painter.galley(pos2(x, icon_y), ig, icon_ink);
    x += TYPE_ICON_SLOT;

    painter.galley(pos2(x, y0), name_g, ink);
    let mut meta_x = x + name_w;
    paint_trailing_marks(painter, &mut meta_x, y0 + name_h / 2.0, pinned, shared, muted);
    painter.galley(pos2(x, y0 + name_h + 2.0), path_g, muted);
    if let Some(n) = shortcut {
        let _ = shortcut_w;
        paint_shortcut_badge(painter, rect.right() - 8.0, rect.center().y, n, muted);
    }
    resp
}

/// Trailing `⌘1` / `Ctrl 1` badge (matches full-screen path search).
fn paint_shortcut_badge(painter: &Painter, right: f32, cy: f32, n: u8, ink: egui::Color32) {
    let modifier = if cfg!(target_os = "macos") { "⌘" } else { "⌃" };
    let label = format!("{modifier}{n}");
    let g = painter.layout_no_wrap(label, FontId::proportional(12.0), ink);
    painter.galley(pos2(right - g.size().x, cy - g.size().y / 2.0), g, ink);
}
