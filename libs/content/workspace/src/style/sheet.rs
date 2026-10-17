//! Task-sheet chrome: dim, canvas panel, header, footer.
//!
//! Shared by desktop sheets and workspace (search folder pick). Body is a
//! canvas plate; footer is Cancel (quiet) + primary.

use egui::{Area, Color32, Id, Layout, Rect, Response, Sense, Ui, pos2, vec2};

use super::button::{Button, icon_button};
use super::chrome::{Radius, Shortcut, control_height, phosphor, shortcut_esc, shortcut_return};
use super::color::Theme;
use super::layout::{
    FixedPadContent, PadContent, claim, origin, place_at, ui_width, with_pad, with_pad_fit,
};
use super::space::Space;
use super::spacer::{Rule, Spacer};
use super::typography::TypeRole;

/// Inner pad of the sheet panel.
const SHEET_PAD: Space = Space::Md;
/// Gap around the footer hairline.
const FOOTER_GAP: Space = Space::Sm;
/// Dim scrim alpha over the shell.
const DIM_ALPHA: u8 = 40;

/// Full-window scrim. Returns `true` if the user clicked the dim **outside**
/// `sheet_layer` (dismiss). Draw the sheet as a **sibling** Foreground area —
/// never nested inside this Area.
pub fn sheet_dim(ctx: &egui::Context, dim_id: Id, sheet_layer: egui::LayerId) -> bool {
    let screen = ctx.screen_rect();
    let mut outside = false;
    Area::new(dim_id)
        .order(egui::Order::Middle)
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

/// Canvas plate for sheet body. `content_w` is the **inner** width (excluding pad).
pub fn sheet_panel(
    ui: &mut Ui, t: &Theme, content_w: f32, content: &mut impl PadContent,
) -> Response {
    let pad = SHEET_PAD.pts();
    super::chrome::plate_content(ui, t.neutral_bg(), t.neutral(), Radius::Surface.corner(), |ui| {
        ui.set_width((content_w + pad * 2.0).max(1.0));
        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
        let mut wrap = SheetPadWrap { content_w: content_w.max(1.0), inner: content };
        with_pad(ui, SHEET_PAD, &mut wrap);
    })
}

/// [`sheet_panel`] when inner height is already known (locked create plate, etc.).
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
pub fn sheet_panel_fit(
    ui: &mut Ui, t: &Theme, content_w: f32, add: impl FnOnce(&mut Ui),
) -> Response {
    let pad = SHEET_PAD.pts();
    let content_w = content_w.max(1.0);
    super::chrome::plate_content(ui, t.neutral_bg(), t.neutral(), Radius::Surface.corner(), |ui| {
        ui.set_width((content_w + pad * 2.0).max(1.0));
        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
        with_pad_fit(ui, SHEET_PAD, add);
    })
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
        let r =
            Rect::from_min_size(rect.min, vec2(self.content_w.min(rect.width()), rect.height()));
        self.inner.place(ui, r);
    }
}

/// Sheet title: **heading** size, secondary ink + trailing dismiss **X**.
///
/// Returns `true` when the X is clicked.
pub fn sheet_title_muted(ui: &mut Ui, t: &Theme, title: &str) -> bool {
    let title_font = TypeRole::Heading.font_id();
    let title_ink = t.neutral_fg_secondary();
    let title_g = ui
        .painter()
        .layout_no_wrap(title.to_owned(), title_font, title_ink);
    let close_sz = control_height();
    let row_h = title_g.size().y.max(close_sz);
    let row_w = ui_width(ui).max(1.0);
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
    let (_, _) = place_at(ui, close_rect, Layout::top_down(egui::Align::Center), |ui| {
        if icon_button(ui, t, phosphor::X, false, t.neutral_bg()).clicked() {
            closed = true;
        }
    });
    claim(ui, outer);
    closed
}

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
    pub accent: bool,
    pub divider: bool,
    pub primary_enabled: bool,
    pub show_primary: bool,
    pub cancel_label: &'static str,
    pub primary_shortcut: Option<Shortcut>,
    pub copy_feedback: Option<&'static str>,
    /// Quiet right action (selection pickers) instead of a solid commit.
    pub quiet_primary: bool,
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
            copy_feedback: None,
            quiet_primary: false,
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

    pub fn back_only(mut self) -> Self {
        self.show_primary = false;
        self.cancel_label = "Back";
        self
    }

    pub fn cancel_label(mut self, label: &'static str) -> Self {
        self.cancel_label = label;
        self
    }

    pub fn copy_feedback(mut self, id: &'static str) -> Self {
        self.copy_feedback = Some(id);
        self
    }

    pub fn primary_shortcut(mut self, s: Shortcut) -> Self {
        self.primary_shortcut = Some(s);
        self
    }

    pub fn quiet_primary(mut self, on: bool) -> Self {
        self.quiet_primary = on;
        self
    }
}

/// Quiet left (esc) · optional primary right.
pub fn sheet_footer(
    ui: &mut Ui, t: &Theme, primary_label: &str, opts: SheetFooterOpts,
) -> SheetFooter {
    if opts.divider {
        ui.add(Spacer::new(FOOTER_GAP));
        ui.add(Rule::new());
        ui.add(Spacer::new(FOOTER_GAP));
    }

    let mut out = SheetFooter::default();
    let row_w = ui_width(ui);
    let row_h = control_height();
    let gap = Space::Sm;
    let primary_sc = opts.primary_shortcut.unwrap_or_else(shortcut_return);
    let top_left = origin(ui);
    let outer = egui::Rect::from_min_size(top_left, vec2(row_w, row_h));

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
    let left_w = cancel_used.width().max(cancel.rect.width());

    if opts.show_primary {
        Spacer::paint_at(
            ui,
            gap,
            egui::Rect::from_min_size(
                pos2(top_left.x + left_w, top_left.y),
                vec2(gap.pts(), row_h),
            ),
        );

        let primary_left = top_left.x + left_w + gap.pts();
        let primary_max = (top_left.x + row_w - primary_left).max(Space::Xl.pts() * 2.4);
        let primary_rect =
            egui::Rect::from_min_size(pos2(primary_left, top_left.y), vec2(primary_max, row_h));
        let _ = place_at(ui, primary_rect, Layout::right_to_left(egui::Align::Center), |ui| {
            ui.set_max_width(primary_max);
            let clicked = if let Some(fid) = opts.copy_feedback {
                Button::quiet(t, primary_label)
                    .enabled(opts.primary_enabled)
                    .copy_feedback(fid)
                    .height(row_h)
                    .max_width(primary_max)
                    .show(ui)
                    .clicked()
            } else if opts.quiet_primary {
                let mut done = Button::quiet(t, primary_label)
                    .enabled(opts.primary_enabled)
                    .height(row_h)
                    .max_width(primary_max);
                if opts.primary_enabled {
                    done = done.shortcut(primary_sc);
                }
                done.show(ui).clicked()
            } else {
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
                primary.show(ui).clicked()
            };
            if clicked {
                out.primary = true;
            }
        });
    }
    claim(ui, outer);

    out
}
