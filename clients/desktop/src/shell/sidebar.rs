//! Sidebar: canvas head + canvas body + surface foot.
//!
//! Layout is **top → bottom · middle**. Resize vs overlay scroll policy lives in
//! [`crate::components::domain::sidebar_resize`].

use egui::{Align, Frame, Id, Layout, Rect, Sense, Ui, UiBuilder, pos2, vec2};

use crate::components::domain::chips;
use crate::components::{
    FixedPadContent, STROKE_HAIRLINE, Space, Spacer, Theme, control_height, show_recents,
    show_shared, show_sync_footer, show_tree, ui_width, with_h_pad_in,
};

use super::ShellApp;
use super::action::Action as A;
use super::action::{Action, SidebarPane};
use super::titlebar::HEADER_H;

pub use crate::components::domain::sidebar_resize::{
    PANEL_ID, WIDTH_DEFAULT, begin_resize_style, end_resize_style, open_t, paint_split_line,
    remember_resting_width, resize_over_workspace, resting_width, restore_panel_width_if_collapsed,
    width_max, width_min,
};

/// Landmarks for the Files head (title clearance + chip row). Tests only.
#[derive(Clone, Debug)]
pub struct SidebarHeadReadout {
    pub header_clearance: Rect,
    pub chip_row: Rect,
    pub chip_mid: Rect,
    pub chip_create: Option<Rect>,
    pub chip_import: Option<Rect>,
    pub chip_search: Option<Rect>,
}

impl Default for SidebarHeadReadout {
    fn default() -> Self {
        Self {
            header_clearance: Rect::NOTHING,
            chip_row: Rect::NOTHING,
            chip_mid: Rect::NOTHING,
            chip_create: None,
            chip_import: None,
            chip_search: None,
        }
    }
}

fn head_readout_id() -> Id {
    Id::new("shell_sidebar_head_readout")
}

#[cfg(test)]
pub fn take_head_readout(ctx: &egui::Context) -> Option<SidebarHeadReadout> {
    ctx.data_mut(|d| d.remove_temp::<SidebarHeadReadout>(head_readout_id()))
}

/// Paint the sidebar at resting width with its **right** edge on this ui's
/// right (slide in/out). Clip so the left runs off-screen.
pub fn show_sliding(
    app: &mut ShellApp, ui: &mut Ui, t: &Theme, queue: &mut Vec<Action>, full_w: f32,
) {
    let vis = ui.max_rect();
    ui.set_clip_rect(vis.intersect(ui.clip_rect()));
    ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
    let full_w = full_w.max(vis.width());
    let full =
        Rect::from_min_size(pos2(vis.right() - full_w, vis.top()), vec2(full_w, vis.height()));
    ui.scope_builder(
        UiBuilder::new()
            .id_salt((PANEL_ID, "slide_body"))
            .max_rect(full)
            .layout(Layout::top_down(Align::Min)),
        |ui| {
            ui.set_min_size(full.size());
            ui.set_max_size(full.size());
            ui.set_clip_rect(vis.intersect(ui.clip_rect()));
            show(app, ui, t, queue);
        },
    );
}

pub fn show(app: &mut ShellApp, ui: &mut Ui, t: &Theme, queue: &mut Vec<Action>) {
    ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
    ui.set_clip_rect(ui.max_rect().intersect(ui.clip_rect()));

    let show_chips = app.pane == SidebarPane::Files;
    egui::TopBottomPanel::top("shell_sidebar_head")
        .resizable(false)
        .show_separator_line(false)
        .frame(Frame::new().fill(t.neutral_bg()).inner_margin(0.0))
        .show_inside(ui, |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
            let mut head = SidebarHeadReadout::default();
            let (clear_rect, _) =
                ui.allocate_exact_size(vec2(ui_width(ui), HEADER_H), Sense::hover());
            head.header_clearance = clear_rect;
            // Same y as the tab-strip hairline — one titleband edge across the window.
            ui.painter().hline(
                clear_rect.x_range(),
                clear_rect.bottom() - STROKE_HAIRLINE * 0.5,
                egui::Stroke::new(STROKE_HAIRLINE, t.neutral()),
            );
            if show_chips {
                ui.add(Spacer::new(Space::Sm));
                let row_h = control_height();
                let mut chips_body = FixedPadContent::new(row_h, |ui| {
                    let (create, import, search, chip_rects) = chips::action_chip_row(ui, t);
                    if create {
                        queue.push(A::Create);
                    }
                    if import {
                        queue.push(A::Import);
                    }
                    if search {
                        queue.push(A::OpenSearch);
                    }
                    head.chip_mid = ui.min_rect();
                    let [a, b, c] = chip_rects;
                    head.chip_create = Some(a);
                    head.chip_import = Some(b);
                    head.chip_search = Some(c);
                });
                let row = with_h_pad_in(ui, Space::Sm, Some(row_h), &mut chips_body);
                head.chip_row = row.rect;
                ui.add(Spacer::new(Space::Md));
            } else {
                ui.add(Spacer::new(Space::Xs));
            }
            ui.ctx()
                .data_mut(|d| d.insert_temp(head_readout_id(), head));
        });

    egui::TopBottomPanel::bottom("shell_sidebar_foot")
        .resizable(false)
        .show_separator_line(false)
        .frame(
            Frame::new()
                .fill(t.neutral_bg_secondary())
                .inner_margin(0.0),
        )
        .show_inside(ui, |ui| {
            ui.set_min_width(ui_width(ui));
            let (r, _) = ui.allocate_exact_size(vec2(ui_width(ui), 1.0), Sense::hover());
            ui.painter().hline(
                r.x_range(),
                r.center().y,
                egui::Stroke::new(STROKE_HAIRLINE, t.neutral()),
            );
            show_sync_footer(app, ui, t, queue);
        });

    egui::CentralPanel::default()
        .frame(Frame::new().fill(t.neutral_bg()).inner_margin(0.0))
        .show_inside(ui, |ui| {
            let body = ui.max_rect();
            ui.set_clip_rect(body.intersect(ui.clip_rect()));
            ui.set_min_size(body.size());
            ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
            match app.pane {
                SidebarPane::Files => show_tree(app, ui, t, queue),
                SidebarPane::Recents => show_recents(app, ui, t, queue),
                SidebarPane::Shared => show_shared(app, ui, t, queue),
            }
        });
}

#[cfg(test)]
mod head_diag {
    use super::*;
    use crate::components::domain::sidebar_resize::{PANEL_ID, width_max, width_min};
    use crate::components::{
        Space, ThemeExt, begin_spacer_record, control_height, install, take_spacer_record,
    };
    use crate::shell::ShellApp;
    use egui::{Context, FullOutput, Pos2, RawInput, SidePanel, Vec2};

    fn fmt_rect(r: Rect) -> String {
        format!(
            "x={:.1}..{:.1} y={:.1}..{:.1}  w={:.1} h={:.1}",
            r.left(),
            r.right(),
            r.top(),
            r.bottom(),
            r.width(),
            r.height()
        )
    }

    fn gap_y(a: Rect, b: Rect) -> f32 {
        // Signed: positive = space between a.bottom and b.top
        b.top() - a.bottom()
    }

    /// Headless dump of every Spacer + landmarks in the Files sidebar head.
    #[test]
    fn diagnose_files_head_spacers() {
        let mut app =
            ShellApp { pane: SidebarPane::Files, sidebar_open: true, ..Default::default() };
        let ctx = Context::default();
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 800.0))),
            ..Default::default()
        };
        let FullOutput { .. } = ctx.run(input.clone(), |ctx| {
            install(ctx);
        });
        let FullOutput { .. } = ctx.run(input, |ctx| {
            install(ctx);
            begin_spacer_record(ctx);
            let t = ctx.get_lb_theme();
            let mut queue = Vec::new();
            SidePanel::left(PANEL_ID)
                .resizable(false)
                .default_width(300.0)
                .width_range(width_min()..=width_max(ctx))
                .show_separator_line(false)
                .frame(Frame::new().inner_margin(0.0))
                .show(ctx, |ui| {
                    show(&mut app, ui, &t, &mut queue);
                });
        });

        let head = take_head_readout(&ctx).expect("head readout");
        let hits = take_spacer_record(&ctx);
        let ch = control_height();
        let sm = Space::Sm.pts();
        let md = Space::Md.pts();
        let xs = Space::Xs.pts();

        eprintln!("=== SIDEBAR FILES HEAD DIAG ===");
        eprintln!("control_height = {ch:.1}  HEADER_H = {HEADER_H:.1}");
        eprintln!("tokens: Xs={xs:.0} Sm={sm:.0} Md={md:.0}");
        eprintln!();
        eprintln!("-- landmarks --");
        eprintln!("header_clearance  {}", fmt_rect(head.header_clearance));
        eprintln!("chip_row          {}", fmt_rect(head.chip_row));
        eprintln!("chip_mid          {}", fmt_rect(head.chip_mid));
        if let Some(r) = head.chip_create {
            eprintln!("chip_create       {}", fmt_rect(r));
        }
        if let Some(r) = head.chip_import {
            eprintln!("chip_import       {}", fmt_rect(r));
        }
        if let Some(r) = head.chip_search {
            eprintln!("chip_search       {}", fmt_rect(r));
        }

        eprintln!();
        eprintln!("-- spacers ({} hits, sorted by top then left) --", hits.len());
        let mut sorted = hits.clone();
        sorted.sort_by(|a, b| {
            a.rect
                .top()
                .partial_cmp(&b.rect.top())
                .unwrap()
                .then(a.rect.left().partial_cmp(&b.rect.left()).unwrap())
        });
        for (i, h) in sorted.iter().enumerate() {
            eprintln!(
                "[{i}] {:?} {} fill_cross={:?} horiz={}  {}",
                h.token,
                if h.horizontal { "H" } else { "V" },
                h.fill_cross,
                h.horizontal,
                fmt_rect(h.rect),
            );
        }

        // Spacers that sit in the head band (above tree): y < chip_row.bottom + Md + 2
        let head_bottom = head.chip_row.bottom() + md + 2.0;
        let head_spacers: Vec<_> = sorted
            .iter()
            .filter(|h| h.rect.top() < head_bottom)
            .cloned()
            .collect();

        eprintln!();
        eprintln!("-- expectations vs reality --");
        let mut fails: Vec<String> = Vec::new();

        // 1. header clearance is HEADER_H and not a Spacer
        if (head.header_clearance.height() - HEADER_H).abs() > 0.5 {
            fails.push(format!(
                "header_clearance.h={:.1} want HEADER_H={HEADER_H:.1}",
                head.header_clearance.height()
            ));
        } else {
            eprintln!("OK  header_clearance height = HEADER_H ({HEADER_H:.1})");
        }

        // 2. vertical Sm above chip row: height Sm, flush under clearance
        let v_sm: Vec<_> = head_spacers
            .iter()
            .filter(|h| !h.horizontal && h.token == Space::Sm)
            .collect();
        if let Some(top_sm) = v_sm.first() {
            let g = gap_y(head.header_clearance, top_sm.rect);
            eprintln!(
                "    top V Sm: h={:.1} (want {sm:.1})  gap after clearance={g:.2}",
                top_sm.rect.height()
            );
            if (top_sm.rect.height() - sm).abs() > 0.5 {
                fails.push(format!("top V Sm height {:.1} != {sm}", top_sm.rect.height()));
            } else {
                eprintln!("OK  top V Sm height");
            }
            if g.abs() > 0.5 {
                fails.push(format!("gap between header_clearance and top V Sm = {g:.2} (want 0)"));
            } else {
                eprintln!("OK  top V Sm flush under header_clearance");
            }
            let g2 = gap_y(top_sm.rect, head.chip_row);
            eprintln!("    gap V Sm → chip_row = {g2:.2} (want 0)");
            if g2.abs() > 0.5 {
                fails.push(format!("gap between top V Sm and chip_row = {g2:.2} (want 0)"));
            } else {
                eprintln!("OK  chip_row flush under top V Sm");
            }
        } else {
            fails.push("no vertical Sm spacer in head".into());
        }

        // 3. chip_row height should be control_height (not inflated by fill_cross)
        eprintln!(
            "    chip_row.h={:.1} chip_mid.h={:.1} control_height={ch:.1}",
            head.chip_row.height(),
            head.chip_mid.height()
        );
        if (head.chip_mid.height() - ch).abs() > 0.5 {
            fails.push(format!(
                "chip_mid.h={:.1} != control_height {ch:.1}",
                head.chip_mid.height()
            ));
        } else {
            eprintln!("OK  chip_mid height = control_height");
        }
        if head.chip_row.height() > ch + 1.0 {
            fails.push(format!(
                "chip_row.h={:.1} > control_height {ch:.1} — likely fill_cross inflated the row",
                head.chip_row.height()
            ));
        } else if (head.chip_row.height() - ch).abs() <= 1.0 {
            eprintln!("OK  chip_row height ≈ control_height");
        }

        // 4. horizontal side pads: Sm wide, height == chip_row
        let h_sm: Vec<_> = head_spacers
            .iter()
            .filter(|h| h.horizontal && h.token == Space::Sm && h.fill_cross.is_some())
            .collect();
        eprintln!("    H Sm fill_cross count = {}", h_sm.len());
        for (i, h) in h_sm.iter().enumerate() {
            eprintln!(
                "    H Sm[{i}]: w={:.1} h={:.1} fill_cross={:?}  {}",
                h.rect.width(),
                h.rect.height(),
                h.fill_cross,
                fmt_rect(h.rect)
            );
            if (h.rect.width() - sm).abs() > 0.5 {
                fails.push(format!("H Sm[{i}] width {:.1} != {sm}", h.rect.width()));
            }
            if h.rect.height() > ch + 1.0 {
                fails.push(format!(
                    "H Sm[{i}] height {:.1} >> control_height {ch:.1}",
                    h.rect.height(),
                ));
            }
            // gap above chip buttons inside the row
            if let Some(c) = head.chip_create {
                let above = c.top() - h.rect.top();
                eprintln!("    H Sm[{i}] top → create.top = {above:.2} (want 0 if flush)");
                if above > 1.0 {
                    fails.push(format!(
                        "invisible air above Create: {above:.1}px (row taller than chip or misaligned)"
                    ));
                }
            }
        }
        if h_sm.len() < 2 {
            fails.push(format!("expected ≥2 H Sm fill_cross side pads, got {}", h_sm.len()));
        } else {
            eprintln!("OK  found side-pad H Sm spacers");
        }

        // 5. chip gaps = Xs between create/import/search
        if let (Some(a), Some(b), Some(c)) = (head.chip_create, head.chip_import, head.chip_search)
        {
            let g1 = b.left() - a.right();
            let g2 = c.left() - b.right();
            eprintln!(
                "    create→import gap={g1:.2}  import→search gap={g2:.2}  (want Xs={xs:.1})"
            );
            if (g1 - xs).abs() > 0.5 || (g2 - xs).abs() > 0.5 {
                fails.push(format!("chip inter-gaps {g1:.1}/{g2:.1} want {xs:.1}"));
            } else {
                eprintln!("OK  chip inter-gaps = Xs");
            }
            // chips same top
            if (a.top() - b.top()).abs() > 0.5 || (b.top() - c.top()).abs() > 0.5 {
                fails.push("chips not top-aligned".into());
            } else {
                eprintln!("OK  chips top-aligned");
            }
            // mid flush to chips top
            let air = a.top() - head.chip_mid.top();
            eprintln!("    chip_mid.top → create.top = {air:.2} (want 0)");
            if air > 1.0 {
                fails.push(format!("air inside chip_mid above Create: {air:.1}"));
            } else {
                eprintln!("OK  chips flush to chip_mid top");
            }
        }

        // 6. bottom Md under chip_row
        let v_md: Vec<_> = head_spacers
            .iter()
            .filter(|h| !h.horizontal && h.token == Space::Md)
            .collect();
        if let Some(bot) = v_md.first() {
            let g = gap_y(head.chip_row, bot.rect);
            eprintln!("    bottom V Md: h={:.1} gap after chip_row={g:.2}", bot.rect.height());
            if (bot.rect.height() - md).abs() > 0.5 {
                fails.push(format!("bottom V Md height {:.1} != {md}", bot.rect.height()));
            } else {
                eprintln!("OK  bottom V Md height");
            }
            if g.abs() > 0.5 {
                fails.push(format!("gap chip_row → V Md = {g:.2}"));
            } else {
                eprintln!("OK  V Md flush under chip_row");
            }
        } else {
            fails.push("no vertical Md under chip row".into());
        }

        // 7. overlapping spacers?
        for i in 0..head_spacers.len() {
            for j in (i + 1)..head_spacers.len() {
                let a = &head_spacers[i];
                let b = &head_spacers[j];
                if a.rect.intersects(b.rect) {
                    let inter = a.rect.intersect(b.rect);
                    if inter.width() > 0.5 && inter.height() > 0.5 {
                        fails.push(format!(
                            "spacers overlap: {:?} {} ∩ {:?} {}",
                            a.token,
                            fmt_rect(a.rect),
                            b.token,
                            fmt_rect(b.rect)
                        ));
                    }
                }
            }
        }
        if fails.iter().all(|f| !f.contains("overlap")) {
            eprintln!("OK  no spacer overlaps in head band");
        }

        eprintln!();
        if fails.is_empty() {
            eprintln!("=== ALL CHECKS PASSED ===");
        } else {
            eprintln!("=== FAILURES ({}) ===", fails.len());
            for f in &fails {
                eprintln!("FAIL  {f}");
            }
        }

        assert!(fails.is_empty(), "sidebar head spacer layout failed:\n{}", fails.join("\n"));
    }
}
