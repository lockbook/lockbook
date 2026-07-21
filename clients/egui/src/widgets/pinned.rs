//! Pinned files section — Apple `PinnedFilesSection` layout for the egui
//! sidebar: a "Pinned" caption and a two-column grid of chips (icon + name),
//! scrollable after three rows. Shown above the Files tree only (not Recents
//! or Shared). Data is a set of ids owned by the shell; this widget only
//! draws and emits open / unpin intents.

use egui::{CornerRadius, FontId, Frame, Margin, Rect, ScrollArea, Sense, Ui, pos2, vec2};
use lb::Uuid;
use workspace_rs::file_cache::FilesExt;
use workspace_rs::show::DocType;
use workspace_rs::widgets::{GlyphonLabel, TextOverflow};
use workspace_rs::GlyphonRendererCallback;

use crate::theme::icons;
use crate::theme::tokens::Tokens;

/// Apple chip height (`chipHeight: 30`).
const CHIP_H: f32 = 30.0;
/// Gap between chips and between grid columns/rows (`spacing: 8`).
const GAP: f32 = 8.0;
/// Chip corner radius (same as action chips / Apple `cornerRadius: 7`).
const RADIUS: u8 = 7;
/// Max rows before the grid scrolls (`maxRows = 3`).
const MAX_ROWS: usize = 3;
const COLS: usize = 2;
/// Horizontal inset (Apple `.padding(.horizontal, 10)`; matches head chip margin).
const PAD_X: f32 = 10.0;

/// Escapes from the pinned section — shell fulfills against workspace / pin set.
#[derive(Clone, Debug)]
pub enum Op {
    /// Open a pinned document or reveal a pinned folder in the tree.
    Open { id: Uuid },
    /// Remove `id` from the pin set (and `lb.unpin_file`).
    Unpin { id: Uuid },
}

/// Draw the pinned section if `pinned` resolves to any live files. Returns at
/// most one `Op` per frame (a click).
pub fn show(
    ui: &mut Ui, t: &Tokens, files: &impl FilesExt, pinned: &std::collections::HashSet<Uuid>,
) -> Option<Op> {
    let mut rows: Vec<&lb::model::file::File> = pinned
        .iter()
        .filter_map(|id| files.get_by_id(*id))
        .collect();
    if rows.is_empty() {
        return None;
    }
    rows.sort_by(|a, b| {
        let (an, bn) = (a.name.to_lowercase(), b.name.to_lowercase());
        an.cmp(&bn).then_with(|| a.id.cmp(&b.id))
    });

    let mut op = None;

    // Surface band (tree body below is canvas). Horizontal pad lines up with
    // the action-chip row above. One GAP below the last chip (frame bottom) —
    // not also a trailing inter-row gap — so bottom air matches caption→chips.
    Frame::new()
        .fill(t.surface())
        .inner_margin(Margin {
            left: PAD_X as i8,
            right: PAD_X as i8,
            top: 0,
            bottom: GAP as i8,
        })
        .show(ui, |ui| {
            // Caption — Apple `.font(.caption).foregroundStyle(.secondary)`.
            let cap = ui.painter().layout_no_wrap(
                "Pinned".into(),
                FontId::proportional(11.0),
                t.text_muted(),
            );
            let (cap_rect, _) =
                ui.allocate_exact_size(vec2(ui.available_width(), cap.size().y), Sense::hover());
            ui.painter().galley(
                pos2(cap_rect.left(), cap_rect.center().y - cap.size().y / 2.0),
                cap,
                t.text_muted(),
            );
            ui.add_space(GAP);

            let row_count = rows.len().div_ceil(COLS);
            let max_h = MAX_ROWS as f32 * CHIP_H + (MAX_ROWS.saturating_sub(1) as f32) * GAP;
            let use_scroll = row_count > MAX_ROWS;

            let draw = |ui: &mut Ui, op: &mut Option<Op>| {
                let w = ui.available_width();
                let chip_w = ((w - GAP) / COLS as f32).max(0.0);
                let chunks: Vec<_> = rows.chunks(COLS).collect();
                for (i, chunk) in chunks.iter().enumerate() {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = GAP;
                        for file in *chunk {
                            ui.scope(|ui| {
                                ui.set_width(chip_w);
                                if let Some(o) = chip(ui, t, file) {
                                    *op = Some(o);
                                }
                            });
                        }
                    });
                    // Gap only *between* rows — bottom frame margin handles trailing air.
                    if i + 1 < chunks.len() {
                        ui.add_space(GAP);
                    }
                }
            };

            if use_scroll {
                crate::widgets::scroll_overlay::with_overlay_scroll(
                    ui,
                    egui::Id::new("pinned_overlay_scroll"),
                    |ui| {
                        let out = ScrollArea::vertical()
                            .id_salt("pinned_scroll")
                            .max_height(max_h)
                            .auto_shrink([false, true])
                            .show(ui, |ui| draw(ui, &mut op));
                        ((), out.state.offset.y)
                    },
                );
            } else {
                draw(ui, &mut op);
            }
        });

    op
}

fn chip(ui: &mut Ui, t: &Tokens, file: &lb::model::file::File) -> Option<Op> {
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(vec2(w, CHIP_H), Sense::click());
    let hover = ui.ctx().animate_bool(resp.id, resp.hovered());
    crate::widgets::nav::paint_chip_chrome(
        ui,
        t,
        rect,
        CornerRadius::same(RADIUS),
        hover,
        resp.is_pointer_button_down_on(),
        t.canvas(),
    );

    let is_folder = file.is_folder();
    let icon = if is_folder {
        icons::FOLDER
    } else {
        icons::for_doc_type(DocType::from_name(&file.name))
    };
    let icon_ink = if is_folder { t.accent() } else { t.fg() };
    let ink = t.fg();

    let icon_g = ui
        .painter()
        .layout_no_wrap(icon.into(), icons::font(12.0), icon_ink);

    let pad_x = 8.0;
    let gap = 5.0;
    let cy = rect.center().y;
    let mut x = rect.left() + pad_x;
    ui.painter()
        .galley(pos2(x, cy - icon_g.size().y / 2.0), icon_g.clone(), icon_ink);
    x += icon_g.size().x + gap;

    // Glyphon for emoji-safe names; ellipsis when the chip is narrow.
    // Clip to the chip interior (past icon) so overflow never paints outside
    // the rounded rect.
    let name_max = (rect.right() - pad_x - x).max(0.0);
    let mut name_truncated = false;
    if name_max > 0.0 {
        let line_h = 18.0_f32;
        let name_rect =
            Rect::from_min_size(pos2(x, cy - line_h / 2.0), vec2(name_max, line_h));
        // Inset clip slightly so glyphs don't kiss the chip edge.
        let chip_text_clip = Rect::from_min_max(
            pos2(x, rect.top()),
            pos2(rect.right() - pad_x, rect.bottom()),
        )
        .intersect(ui.clip_rect());
        let clip = chip_text_clip.intersect(name_rect);
        if clip.width() > 0.0 && clip.height() > 0.0 {
            let full_w = GlyphonLabel::new(&file.name, ink)
                .font_size(13.0)
                .line_height(line_h)
                .max_width(f32::MAX)
                .measure(ui)
                .x;
            name_truncated = full_w > name_max + 0.5;
            let shaped = GlyphonLabel::new(&file.name, ink)
                .font_size(13.0)
                .line_height(line_h)
                .max_width(name_max)
                .text_overflow(TextOverflow::EndEllipsis)
                .build(ui.ctx());
            let area = shaped.text_area(name_rect, ui.ctx(), clip);
            ui.painter().add(
                egui_wgpu_renderer::egui_wgpu::Callback::new_paint_callback(
                    clip,
                    GlyphonRendererCallback::new(vec![area]),
                ),
            );
        }
    }

    if name_truncated {
        workspace_rs::widgets::tip_text(ui.ctx(), &resp, &file.name);
    }

    let mut op = None;
    if resp.clicked() {
        op = Some(Op::Open { id: file.id });
    }
    if let Some(unpin) = crate::widgets::context_menu::show(&resp, t, |m| {
        m.item(icons::PUSH_PIN_SLASH, "Unpin", Op::Unpin { id: file.id });
    }) {
        op = Some(unpin);
    }
    op
}
