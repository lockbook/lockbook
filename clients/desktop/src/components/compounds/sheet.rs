//! Task-sheet chrome: dim, canvas panel, header, footer.
//!
//! Shared by Share / Move / Delete / Create — not a full product modal.
//! Sheet body is a **canvas plate**; footer is Cancel (quiet) + primary.

use egui::{Area, Color32, Id, Layout, Order, Rect, Response, Sense, Ui, pos2, vec2};

use crate::components::atoms::button::{Button, icon_button};
use crate::components::foundation::chrome::{
    Radius, Shortcut, control_height, phosphor, shortcut_esc, shortcut_return,
};
use crate::components::foundation::color::Theme;
use crate::components::foundation::layout::{
    FixedPadContent, PadContent, claim, origin, place_at, with_pad,
};
use crate::components::foundation::space::Space;
use crate::components::foundation::spacer::{Rule, Spacer};
use crate::components::foundation::typography::TypeRole;

/// Inner pad of the sheet panel.
const SHEET_PAD: Space = Space::Md;
/// Gap around the footer hairline.
const FOOTER_GAP: Space = Space::Sm;
/// Dim scrim alpha over the shell.
const DIM_ALPHA: u8 = 40;

// ── Dim ─────────────────────────────────────────────────────────────────────

/// Full-window scrim. Returns `true` if the user clicked the dim **outside**
/// `sheet_layer` (dismiss). Draw the sheet as a **sibling** Foreground area —
/// never nested inside this Area.
pub fn sheet_dim(ctx: &egui::Context, dim_id: Id, sheet_layer: egui::LayerId) -> bool {
    let screen = ctx.screen_rect();
    let mut outside = false;
    Area::new(dim_id)
        .order(Order::Middle)
        .fixed_pos(screen.min)
        .default_size(screen.size())
        .fade_in(false)
        .sense(Sense::click())
        .show(ctx, |ui| {
            let (rect, resp) = ui.allocate_exact_size(screen.size(), Sense::click());
            ui.painter()
                .rect_filled(rect, 0.0, Color32::from_black_alpha(DIM_ALPHA));
            if resp.clicked() {
                let on_sheet = ctx
                    .pointer_interact_pos()
                    .is_some_and(|pos| ctx.layer_id_at(pos) == Some(sheet_layer));
                if !on_sheet {
                    outside = true;
                }
            }
        });
    outside
}

// ── Panel ───────────────────────────────────────────────────────────────────

/// Canvas plate for sheet body. `content_w` is the **inner** width (excluding pad).
///
/// `content` is [`PadContent`]: measure height at `content_w`, then place.
pub fn sheet_panel(
    ui: &mut Ui, t: &Theme, content_w: f32, content: &mut impl PadContent,
) -> Response {
    let pad = SHEET_PAD.pts();
    // Outside hairline — not Frame::stroke (Inside; flush kids cover it).
    crate::components::foundation::chrome::plate_content(
        ui,
        t.neutral_bg(),
        t.neutral(),
        Radius::Surface.corner(),
        |ui| {
            ui.set_width((content_w + pad * 2.0).max(1.0));
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            // Adapter: pad measures/places at full outer mid width; content
            // was measured for content_w — set width inside place.
            let mut wrap = SheetPadWrap { content_w: content_w.max(1.0), inner: content };
            with_pad(ui, SHEET_PAD, &mut wrap);
        },
    )
}

/// [`sheet_panel`] when inner height is already known (locked create plate, etc.).
///
/// **Do not** pass a guessed “big enough” height (480, 1600, …) — that becomes
/// scroll extent / empty plate. Prefer [`sheet_panel_fit`] for content-sized
/// dialogs, or a **measured** lock (create sheet) for stable multi-step plates.
pub fn sheet_panel_fixed(
    ui: &mut Ui, t: &Theme, content_w: f32, content_h: f32, add: impl FnOnce(&mut Ui),
) -> Response {
    let mut body = FixedPadContent::new(content_h.max(1.0), |ui| {
        ui.set_width(content_w.max(1.0));
        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
        add(ui);
    });
    sheet_panel(ui, t, content_w, &mut body)
}

/// Content-sized sheet: mid height from layout, not a hardcoded guess.
///
/// Same pad / Outside hairline as [`sheet_panel_fixed`]. `max_rect` budget is
/// only for measure — outer size claims **used** height (+ pad).
pub fn sheet_panel_fit(
    ui: &mut Ui, t: &Theme, content_w: f32, add: impl FnOnce(&mut Ui),
) -> Response {
    let pad = SHEET_PAD.pts();
    let content_w = content_w.max(1.0);
    crate::components::foundation::chrome::plate_content(
        ui,
        t.neutral_bg(),
        t.neutral(),
        Radius::Surface.corner(),
        |ui| {
            ui.set_width((content_w + pad * 2.0).max(1.0));
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            crate::components::foundation::layout::with_pad_fit(ui, SHEET_PAD, add);
        },
    )
}

struct SheetPadWrap<'a, C: PadContent> {
    content_w: f32,
    inner: &'a mut C,
}

impl<C: PadContent> PadContent for SheetPadWrap<'_, C> {
    fn measure(&self, ui: &Ui, _width: f32) -> f32 {
        self.inner.measure(ui, self.content_w)
    }

    fn place(&mut self, ui: &mut Ui, rect: Rect) {
        // Mid rect may be wider than content_w when outer width is forced; pin width.
        let r =
            Rect::from_min_size(rect.min, vec2(self.content_w.min(rect.width()), rect.height()));
        self.inner.place(ui, r);
    }
}

// ── Header ──────────────────────────────────────────────────────────────────

/// Sheet title: **heading** size, secondary ink + trailing dismiss **X**.
///
/// Returns `true` when the X is clicked. Same dismiss affordance as Cancel /
/// Esc / corner close control.
pub fn sheet_title_muted(ui: &mut Ui, t: &Theme, title: &str) -> bool {
    sheet_title_bar(ui, t, title)
}

fn sheet_title_bar(ui: &mut Ui, t: &Theme, title: &str) -> bool {
    // Layout heading — between body and page title; quiet secondary ink.
    let title_font = TypeRole::Heading.font_id();
    let title_ink = t.neutral_fg_secondary();
    let title_g = ui
        .painter()
        .layout_no_wrap(title.to_owned(), title_font, title_ink);
    let close_sz = control_height();
    let row_h = title_g.size().y.max(close_sz);
    let row_w = crate::components::ui_width(ui).max(1.0);
    let top_left = origin(ui);
    let outer = egui::Rect::from_min_size(top_left, vec2(row_w, row_h));

    ui.painter().galley(
        pos2(top_left.x, top_left.y + (row_h - title_g.size().y) / 2.0),
        title_g,
        title_ink,
    );

    let close_left = top_left.x + row_w - close_sz;

    let mut closed = false;
    let close_rect = egui::Rect::from_min_size(pos2(close_left, top_left.y), vec2(close_sz, row_h));
    // Sheet is canvas (`neutral_bg`); ghost hover wash uses that ground.
    let (_, _) = place_at(ui, close_rect, Layout::top_down(egui::Align::Center), |ui| {
        if icon_button(ui, t, phosphor::X, false, t.neutral_bg()).clicked() {
            closed = true;
        }
    });
    claim(ui, outer);
    closed
}

// ── Footer ──────────────────────────────────────────────────────────────────

/// Result of the Cancel | Primary footer.
#[derive(Clone, Copy, Debug, Default)]
pub struct SheetFooter {
    pub cancel: bool,
    pub primary: bool,
}

/// Options for [`sheet_footer`].
#[derive(Clone, Copy, Debug)]
pub struct SheetFooterOpts {
    pub danger: bool,
    /// Solid accent primary (same plate recipe as danger; brand hue). Mutually
    /// exclusive with [`Self::danger`] — danger wins if both set.
    pub accent: bool,
    pub divider: bool,
    pub primary_enabled: bool,
    /// When false, only the quiet left button is shown (e.g. Back / Esc).
    pub show_primary: bool,
    /// Quiet left label; default `"Cancel"`.
    pub cancel_label: &'static str,
    /// Badge on the primary; default [`shortcut_return`] (⌘⏎).
    pub primary_shortcut: Option<Shortcut>,
}

impl Default for SheetFooterOpts {
    fn default() -> Self {
        Self {
            danger: false,
            accent: false,
            divider: true,
            primary_enabled: true,
            show_primary: true,
            cancel_label: "Cancel",
            primary_shortcut: None,
        }
    }
}

impl SheetFooterOpts {
    pub fn danger(mut self, on: bool) -> Self {
        self.danger = on;
        self
    }

    pub fn accent(mut self, on: bool) -> Self {
        self.accent = on;
        self
    }

    pub fn divider(mut self, on: bool) -> Self {
        self.divider = on;
        self
    }

    pub fn primary_enabled(mut self, on: bool) -> Self {
        self.primary_enabled = on;
        self
    }

    /// Quiet-only footer (no primary). `primary_label` on [`sheet_footer`] is ignored.
    pub fn back_only(mut self) -> Self {
        self.show_primary = false;
        self.cancel_label = "Back";
        self
    }

    /// Override primary kbd badge (e.g. plain ⏎ on create).
    pub fn primary_shortcut(mut self, s: Shortcut) -> Self {
        self.primary_shortcut = Some(s);
        self
    }
}

/// Quiet left (esc) · optional primary right.
///
/// When [`SheetFooterOpts::show_primary`] is false, only the left button is
/// painted (`cancel_label`, default Cancel; use [`SheetFooterOpts::back_only`]).
pub fn sheet_footer(
    ui: &mut Ui, t: &Theme, primary_label: &str, opts: SheetFooterOpts,
) -> SheetFooter {
    if opts.divider {
        // Spacers around a zero-height rule — F2 shows the air, Rule is the hairline.
        ui.add(Spacer::new(FOOTER_GAP));
        ui.add(Rule::new());
        ui.add(Spacer::new(FOOTER_GAP));
    }

    let mut out = SheetFooter::default();
    let row_w = crate::components::ui_width(ui);
    let row_h = control_height();
    let gap = Space::Sm;
    let primary_sc = opts.primary_shortcut.unwrap_or_else(shortcut_return);
    let top_left = origin(ui);
    let outer = egui::Rect::from_min_size(top_left, vec2(row_w, row_h));

    // Quiet dismiss at left of row band.
    let cancel_slot = egui::Rect::from_min_size(top_left, vec2(row_w, row_h));
    let (cancel, cancel_used) =
        place_at(ui, cancel_slot, Layout::top_down(egui::Align::Min), |ui| {
            Button::quiet(t, opts.cancel_label)
                .shortcut(shortcut_esc())
                .height(row_h)
                .show(ui)
        });
    if cancel.clicked() {
        out.cancel = true;
    }
    let cancel_w = cancel_used.width().max(cancel.rect.width());

    if opts.show_primary {
        Spacer::paint_at(
            ui,
            gap,
            egui::Rect::from_min_size(
                pos2(top_left.x + cancel_w, top_left.y),
                vec2(gap.pts(), row_h),
            ),
        );

        // Primary flush-right in the remaining band.
        let primary_left = top_left.x + cancel_w + gap.pts();
        let primary_max = (top_left.x + row_w - primary_left).max(Space::Xl.pts() * 2.4);
        let primary_rect =
            egui::Rect::from_min_size(pos2(primary_left, top_left.y), vec2(primary_max, row_h));
        let _ = place_at(ui, primary_rect, Layout::right_to_left(egui::Align::Center), |ui| {
            ui.set_max_width(primary_max);
            let mut primary = if opts.danger {
                Button::primary(t, primary_label).danger()
            } else if opts.accent {
                Button::primary(t, primary_label).accent()
            } else {
                Button::primary(t, primary_label)
            }
            .enabled(opts.primary_enabled)
            .height(row_h)
            .max_width(primary_max);
            if opts.primary_enabled {
                primary = primary.shortcut(primary_sc);
            }
            if primary.show(ui).clicked() {
                out.primary = true;
            }
        });
    }
    claim(ui, outer);

    out
}

// ── Measure+place section helpers (Tier B) ──────────────────────────────────

/// Place content in a parent-owned band of height `h` (full current ui width).
///
/// Top-down Min — no residual `available_height` fill. Parent claims the band.
pub fn sheet_band(ui: &mut Ui, h: f32, add: impl FnOnce(&mut Ui)) {
    let h = h.max(1.0);
    let w = crate::components::ui_width(ui);
    let top_left = origin(ui);
    let band = egui::Rect::from_min_size(top_left, vec2(w, h));
    let _ = place_at(ui, band, Layout::top_down(egui::Align::Min), |ui| {
        ui.set_width(w);
        ui.set_min_height(h);
        ui.set_height(h);
        ui.set_max_height(h);
        add(ui);
    });
    claim(ui, band);
}

/// Center `add` inside a fixed band (empty states, spinners) without
/// `vertical_centered` on an unconstrained Area.
pub fn sheet_band_centered(ui: &mut Ui, h: f32, add: impl FnOnce(&mut Ui)) {
    let h = h.max(1.0);
    let w = crate::components::ui_width(ui);
    let top_left = origin(ui);
    let band = egui::Rect::from_min_size(top_left, vec2(w, h));
    let _ = place_at(ui, band, Layout::top_down(egui::Align::Center), |ui| {
        ui.set_width(w);
        ui.with_layout(Layout::top_down(egui::Align::Center), add);
    });
    claim(ui, band);
}

/// Place equal-width (or pre-measured) cells in one row at absolute x.
///
/// `widths[i]` is cell width; gaps use [`super::super::atoms::chip_layout::CHIP_GAP`]
/// via `gap_pts` / paint. Caller owns total claim via this helper.
pub fn sheet_equal_row(
    ui: &mut Ui, heights: f32, widths: &[f32], gap: Space, mut cell: impl FnMut(&mut Ui, usize),
) {
    use crate::components::foundation::spacer::Spacer;
    let h = heights.max(1.0);
    let gap_pts = gap.pts();
    let top_left = origin(ui);
    let mut x = top_left.x;
    let mut total_w = 0.0_f32;
    for (i, &w) in widths.iter().enumerate() {
        if i > 0 {
            Spacer::paint_at(
                ui,
                gap,
                egui::Rect::from_min_size(pos2(x, top_left.y), vec2(gap_pts, h)),
            );
            x += gap_pts;
            total_w += gap_pts;
        }
        let cell_r = egui::Rect::from_min_size(pos2(x, top_left.y), vec2(w.max(0.0), h));
        let _ = place_at(ui, cell_r, Layout::top_down(egui::Align::Min), |ui| {
            ui.set_width(w.max(0.0));
            ui.set_height(h);
            cell(ui, i);
        });
        x += w.max(0.0);
        total_w += w.max(0.0);
    }
    claim(ui, egui::Rect::from_min_size(top_left, vec2(total_w.max(1.0), h)));
}
