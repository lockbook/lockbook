//! Pinned section: caption + two-column chip grid on surface.
//!
//! Chips are canvas plates; hover is ground-relative ink wash (no outline).
//! Layout air uses [`Spacer`] so F2 space overlay paints token bands.
//! Right-click: file menu (same intents as tree/recents; always **Unpin**).

use egui::{Id, Ui, pos2, vec2};
use lb::Uuid;
use workspace_rs::file_cache::FilesExt;

use crate::components::{
    EqualCells, FixedPadContent, LIST_PAD, QuietChipAlign, QuietChipLabel, Space, Spacer, Theme,
    TypeRole, context_menu, file_row_icon, phosphor, quiet_chip, quiet_chip_height, with_h_pad,
    with_overlay_scroll,
};

use crate::shell::action::Action;
use crate::shell::action::Action as A;

const COLS: usize = 2;
const MAX_ROWS: usize = 3;

fn chip_h() -> f32 {
    quiet_chip_height()
}

/// Pin-chip menu — subset of tree file commands that make sense off the strip.
#[derive(Clone, Copy)]
enum PinCmd {
    Open,
    OpenNewTab,
    Create,
    ExpandAll,
    CollapseAll,
    Rename,
    Unpin,
    Move,
    Duplicate,
    Share,
    CopyLink,
    Export,
    Delete,
}

struct PinRow {
    id: Uuid,
    name: String,
    is_folder: bool,
    /// Create destination: folder id, or parent of a document.
    create_parent: Uuid,
}

pub fn show(
    ui: &mut Ui, t: &Theme, files: &impl FilesExt, pinned: &[Uuid], queue: &mut Vec<Action>,
) {
    let mut rows: Vec<PinRow> = pinned
        .iter()
        .filter_map(|id| {
            let f = files.get_by_id(*id)?;
            let create_parent = if f.is_folder() { f.id } else { f.parent };
            Some(PinRow { id: *id, name: f.name.clone(), is_folder: f.is_folder(), create_parent })
        })
        .collect();
    if rows.is_empty() {
        return;
    }
    rows.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

    ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);

    // Surface band: pad via Spacers (F2), not Frame margin.
    egui::Frame::new()
        .fill(t.neutral_bg_secondary())
        .inner_margin(0.0)
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
            // Equal L/R LIST_PAD Spacers (same helper as Files tree / action chips).
            let row_count = rows.len().div_ceil(COLS);
            let ch = chip_h();
            let max_h =
                MAX_ROWS as f32 * ch + (MAX_ROWS.saturating_sub(1) as f32) * EqualCells::gap_pts();
            let use_scroll = row_count > MAX_ROWS;
            let grid_h = if use_scroll {
                max_h
            } else {
                let n = row_count.max(1) as f32;
                n * ch + (row_count.saturating_sub(1) as f32) * EqualCells::gap_pts()
            };
            let pins_h = 14.0 + Space::Sm.pts() + grid_h;
            let mut pins_body = FixedPadContent::new(pins_h, |ui| {
                ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);

                ui.label(
                    TypeRole::Body
                        .rich("Pinned")
                        .color(t.neutral_fg_secondary())
                        .size(11.0),
                );
                ui.add(Spacer::new(Space::Sm));

                let mut draw = |ui: &mut Ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
                    // Measure once per row; place cells at absolute x (no horizontal placer).
                    let row_w = crate::components::ui_width(ui);
                    let cells = EqualCells::measure(row_w, COLS);
                    let gap = EqualCells::gap_pts();
                    let chunks: Vec<_> = rows.chunks(COLS).collect();
                    let ch = chip_h();
                    for (i, chunk) in chunks.iter().enumerate() {
                        if i > 0 {
                            ui.add(EqualCells::gap_spacer());
                        }
                        let top_left = crate::components::origin(ui);
                        let mut x = top_left.x;
                        for (j, row) in chunk.iter().enumerate() {
                            if j > 0 {
                                Spacer::paint_at(
                                    ui,
                                    EqualCells::gap_token(),
                                    egui::Rect::from_min_size(pos2(x, top_left.y), vec2(gap, ch)),
                                );
                                x += gap;
                            }
                            let cell = egui::Rect::from_min_size(
                                pos2(x, top_left.y),
                                vec2(cells.cell_w, ch),
                            );
                            let _ = crate::components::place_at(
                                ui,
                                cell,
                                egui::Layout::top_down(egui::Align::Min),
                                |ui| {
                                    ui.set_width(cells.cell_w);
                                    pin_chip(ui, t, row, queue);
                                },
                            );
                            x += cells.cell_w;
                        }
                        crate::components::claim(
                            ui,
                            egui::Rect::from_min_size(top_left, vec2(row_w, ch)),
                        );
                    }
                };

                if use_scroll {
                    let scroll_id = Id::new("pinned_scroll");
                    with_overlay_scroll(ui, scroll_id, |ui| {
                        let out = egui::ScrollArea::vertical()
                            .id_salt("pinned_scroll")
                            .max_height(max_h)
                            .auto_shrink([false, true])
                            .show(ui, draw);
                        ((), out.state.offset.y, out.id)
                    });
                } else {
                    draw(ui);
                }
            });
            with_h_pad(ui, LIST_PAD, &mut pins_body);
            ui.add(Spacer::new(Space::Sm)); // bottom of band
        });
}

fn pin_chip(ui: &mut Ui, t: &Theme, row: &PinRow, queue: &mut Vec<Action>) {
    let icon = file_row_icon(&row.name, row.is_folder);
    let icon_ink = if row.is_folder { t.accent() } else { t.neutral_fg() };
    let resp = quiet_chip(
        ui,
        t,
        icon,
        icon_ink,
        Some(QuietChipLabel::FileName(&row.name)),
        QuietChipAlign::Start,
    );

    if resp.clicked() {
        if row.is_folder {
            // Reveal/select in the tree; expand so contents are visible.
            queue.push(A::SelectFile(row.id));
            queue.push(A::ExpandSubtree(row.id));
        } else {
            queue.push(A::OpenFile(row.id));
        }
    }
    if resp.secondary_clicked() {
        queue.push(A::SelectFile(row.id));
    }

    if let Some(cmd) = context_menu::show(&resp, t, |e| {
        if !row.is_folder {
            e.item(phosphor::ARROW_SQUARE_OUT, "Open", PinCmd::Open);
            e.item(phosphor::APP_WINDOW, "Open in new tab", PinCmd::OpenNewTab);
            e.separator();
        }
        e.item(phosphor::NOTE_PENCIL, "Create…", PinCmd::Create);
        if row.is_folder {
            e.separator();
            e.item(phosphor::CARET_DOWN, "Expand all", PinCmd::ExpandAll);
            e.item(phosphor::CARET_LEFT, "Collapse all", PinCmd::CollapseAll);
        }
        e.separator();
        e.item(phosphor::PENCIL, "Rename…", PinCmd::Rename);
        e.item(phosphor::PUSH_PIN, "Unpin", PinCmd::Unpin);
        e.item(phosphor::FOLDERS, "Move…", PinCmd::Move);
        if !row.is_folder {
            e.item(phosphor::COPY, "Duplicate", PinCmd::Duplicate);
        }
        e.separator();
        e.item(phosphor::USERS, "Share…", PinCmd::Share);
        if !row.is_folder {
            e.item(phosphor::LINK, "Copy link", PinCmd::CopyLink);
            e.item(phosphor::DOWNLOAD_SIMPLE, "Export…", PinCmd::Export);
        }
        e.separator();
        e.item_danger(phosphor::TRASH, "Delete…", PinCmd::Delete);
    }) {
        match cmd {
            PinCmd::Open => queue.push(A::OpenFile(row.id)),
            PinCmd::OpenNewTab => queue.push(A::OpenFileNewTab(row.id)),
            PinCmd::Create => {
                queue.push(A::OpenCreate { parent: Some(row.create_parent), is_folder: false })
            }
            PinCmd::ExpandAll => queue.push(A::ExpandSubtree(row.id)),
            PinCmd::CollapseAll => queue.push(A::CollapseSubtree(row.id)),
            PinCmd::Rename => queue.push(A::OpenRename(row.id)),
            PinCmd::Unpin => queue.push(A::TogglePin(row.id)),
            PinCmd::Move => queue.push(A::OpenMove(vec![row.id])),
            PinCmd::Duplicate => queue.push(A::Duplicate(vec![row.id])),
            PinCmd::Share => queue.push(A::OpenShare(row.id)),
            PinCmd::CopyLink => queue.push(A::CopyLink(row.id)),
            PinCmd::Export => queue.push(A::Export(vec![row.id])),
            PinCmd::Delete => queue.push(A::OpenDelete(vec![row.id])),
        }
    }
}
