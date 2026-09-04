//! System Settings–style modal: rail + form pages.
//!
//! ## Spacing (all F2-visible [`Spacer`]s)
//!
//! ```text
//! plate
//! ├─ header band = Sm · row(control) · Sm     (L/R Sm on the row)
//! └─ body
//!    ├─ rail: Sm top · [Sm | nav | Sm]     (secondary, continuous with header)
//!    └─ content: canvas + L hairline (top + left) — same idea as shell
//!         tab strip vs workspace (separator only under the strip)
//!         Lg · [Lg | heading · Md · page | Lg] · Lg
//!           page: section_label · Xs · form_group · …
//!           form_group: Sm · row · (Xs · row)* · Sm
//!           form_row: Md | label · trailing | Sm   (height = control)
//!           form_row detail: label · Xs · detail
//!           sections: Lg between groups
//! ```

use crate::components::{
    Button, FixedPadContent, NavItem, Radius, STROKE_HAIRLINE, Space, Spacer, Theme, ThemeFamily,
    TypeRole, control_height, form_group, form_picker, form_row, form_segmented, form_toggle,
    form_toggle_detail, form_value, icon_button, paint_plate, phosphor, section_label, sheet_dim,
    ui_width, with_h_pad, with_overlay_scroll,
};
use egui::{
    Align, Area, CornerRadius, Id, Label, Layout, Order, Pos2, Rect, ScrollArea, Sense, Stroke,
    TextWrapMode, Ui, UiBuilder, vec2,
};

use super::ShellApp;
use super::action::Action as A;
use super::action::{Action, Modal, SettingsCat};

pub use crate::components::domain::settings_plate::{
    plate_origin_for_screen, plate_size_for_screen,
};

const RAIL_W: f32 = 188.0;

/// Geometry after a settings layout pass (tests + diagnostics).
#[derive(Clone, Debug)]
pub struct SettingsLayoutReadout {
    /// Full viewport used for the pass (asserted in layout tests).
    #[allow(dead_code)]
    pub screen: Rect,
    pub plate: Rect,
    pub header: Rect,
    pub body: Rect,
    pub rail: Rect,
    pub content: Rect,
    pub page_heading: Option<Rect>,
    pub first_nav: Option<Rect>,
}

impl Default for SettingsLayoutReadout {
    fn default() -> Self {
        Self {
            screen: Rect::NOTHING,
            plate: Rect::NOTHING,
            header: Rect::NOTHING,
            body: Rect::NOTHING,
            rail: Rect::NOTHING,
            content: Rect::NOTHING,
            page_heading: None,
            first_nav: None,
        }
    }
}

fn layout_readout_id() -> Id {
    Id::new("shell_settings_layout_readout")
}

/// Last frame’s settings geometry (headless tests read this after `show`).
#[cfg(test)]
pub fn take_layout_readout(ctx: &egui::Context) -> Option<SettingsLayoutReadout> {
    ctx.data_mut(|d| d.remove_temp::<SettingsLayoutReadout>(layout_readout_id()))
}

/// Header content row (title + close) — same ladder as controls.
fn header_row_h() -> f32 {
    control_height().max(TypeRole::Heading.line_height())
}

/// Title bar: `Spacer Sm` · row · `Spacer Sm` (all F2-visible; not Align-center air).
fn header_band_h() -> f32 {
    Space::Sm.pts() * 2.0 + header_row_h()
}

pub fn show(app: &mut ShellApp, ctx: &egui::Context, t: &Theme, queue: &mut Vec<Action>) {
    let cat = match &app.modal {
        Some(Modal::Settings { cat }) => *cat,
        _ => return,
    };

    let layer = egui::LayerId::new(Order::Foreground, Id::new("shell_settings"));
    if sheet_dim(ctx, Id::new("shell_settings_dim"), layer) {
        queue.push(A::CloseModal);
    }

    let screen = ctx.screen_rect();
    let (settings_w, settings_h) = plate_size_for_screen(screen);
    let plate_origin = plate_origin_for_screen(screen, settings_w, settings_h);

    let mut readout = SettingsLayoutReadout {
        screen,
        plate: Rect::NOTHING,
        header: Rect::NOTHING,
        body: Rect::NOTHING,
        rail: Rect::NOTHING,
        content: Rect::NOTHING,
        page_heading: None,
        first_nav: None,
    };

    Area::new(Id::new("shell_settings"))
        .order(Order::Foreground)
        // Pivot = top-left; we already did the center/clamp math.
        .fixed_pos(plate_origin)
        .show(ctx, |ui| {
            let (plate_rect, _) =
                ui.allocate_exact_size(vec2(settings_w, settings_h), Sense::hover());
            readout.plate = plate_rect;

            // Outside stroke lives outside the fill, so rail/content paints
            // (flush to plate_rect) cannot cover it — draw once, up front.
            paint_plate(
                ui,
                plate_rect,
                Radius::Surface.corner(),
                t.neutral_bg_secondary(),
                t.neutral(),
            );

            let header_h = header_band_h().min(settings_h * 0.45).max(1.0);
            let header_rect = Rect::from_min_size(plate_rect.min, vec2(settings_w, header_h));
            let body_rect = Rect::from_min_max(
                Pos2::new(plate_rect.left(), header_rect.bottom()),
                plate_rect.max,
            );
            readout.header = header_rect;
            readout.body = body_rect;

            // ── Header ─────────────────────────────────────────────────────
            // Stack (F2): Sm · [Sm | title … close | Sm] · Sm — absolute place.
            ui.scope_builder(UiBuilder::new().max_rect(header_rect), |ui| {
                ui.set_clip_rect(header_rect.intersect(ui.clip_rect()));
                ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
                let pad = Space::Sm;
                let p = pad.pts();
                let row_h = header_row_h();
                let origin = header_rect.min;
                // Top / bottom pad bands
                Spacer::paint_at(ui, pad, Rect::from_min_size(origin, vec2(settings_w, p)));
                Spacer::paint_at(
                    ui,
                    pad,
                    Rect::from_min_size(
                        Pos2::new(origin.x, origin.y + p + row_h),
                        vec2(settings_w, p),
                    ),
                );
                let row_top = origin.y + p;
                Spacer::paint_at(
                    ui,
                    pad,
                    Rect::from_min_size(Pos2::new(origin.x, row_top), vec2(p, row_h)),
                );
                Spacer::paint_at(
                    ui,
                    pad,
                    Rect::from_min_size(
                        Pos2::new(origin.x + settings_w - p, row_top),
                        vec2(p, row_h),
                    ),
                );
                let mid = Rect::from_min_size(
                    Pos2::new(origin.x + p, row_top),
                    vec2((settings_w - p * 2.0).max(1.0), row_h),
                );
                // Title left, close right — no horizontal Center residual.
                let title_g = ui.painter().layout_no_wrap(
                    "Settings".into(),
                    TypeRole::Heading.font_id(),
                    t.neutral_fg_secondary(),
                );
                ui.painter().galley(
                    Pos2::new(mid.left(), mid.center().y - title_g.size().y / 2.0),
                    title_g,
                    t.neutral_fg_secondary(),
                );
                let close_sz = control_height();
                let close_r = Rect::from_min_size(
                    Pos2::new(mid.right() - close_sz, mid.center().y - close_sz / 2.0),
                    vec2(close_sz, close_sz),
                );
                let _ = crate::components::place_at(
                    ui,
                    close_r,
                    Layout::top_down(Align::Center),
                    |ui| {
                        if icon_button(ui, t, phosphor::X, false, t.neutral_bg_secondary())
                            .clicked()
                        {
                            queue.push(A::CloseModal);
                        }
                    },
                );
            });

            // ── Body: rail + content (exact rects) ─────────────────────────
            let body_h = body_rect.height().max(1.0);
            let body_w = body_rect.width().max(1.0);
            let r = Radius::Surface.pts();
            let rail_radius = CornerRadius { nw: 0, ne: 0, sw: r, se: 0 };
            let content_radius = CornerRadius { nw: 0, ne: 0, sw: 0, se: r };

            let rail_w = RAIL_W.min(body_w * 0.45).max(1.0);
            let content_w = (body_w - rail_w).max(1.0);
            let rail_rect = Rect::from_min_size(body_rect.min, vec2(rail_w, body_h));
            let content_rect =
                Rect::from_min_size(body_rect.min + vec2(rail_w, 0.0), vec2(content_w, body_h));
            readout.rail = rail_rect;
            readout.content = content_rect;

            ui.painter()
                .rect_filled(rail_rect, rail_radius, t.neutral_bg_secondary());
            ui.painter()
                .rect_filled(content_rect, content_radius, t.neutral_bg());

            // ── Rail: Spacer Sm · with_h_pad(Sm) content-driven nav stack
            ui.scope_builder(UiBuilder::new().max_rect(rail_rect), |ui| {
                ui.set_clip_rect(rail_rect.intersect(ui.clip_rect()));
                ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
                let pad = Space::Sm;
                ui.add(Spacer::new(pad));
                let nav_h = SettingsCat::ALL.len() as f32 * control_height();
                let mut nav = FixedPadContent::new(nav_h, |ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
                    for c in SettingsCat::ALL {
                        let resp = NavItem::new(t, c.title(), c.icon())
                            .selected(c == cat)
                            .show(ui);
                        if readout.first_nav.is_none() {
                            readout.first_nav = Some(resp.rect);
                        }
                        if resp.clicked() {
                            queue.push(A::SetSettingsCat(c));
                        }
                    }
                });
                with_h_pad(ui, pad, &mut nav);
            });

            // Scroll content is clipped *inside* the L-hairlines so glyphs don’t
            // paint over the stroke (bleed stayed in the plate but looked wrong).
            let hair = STROKE_HAIRLINE;
            let scroll_slot = Rect::from_min_max(
                Pos2::new(content_rect.left() + hair, content_rect.top() + hair),
                content_rect.max,
            );

            ui.scope_builder(UiBuilder::new().max_rect(content_rect), |ui| {
                ui.set_clip_rect(scroll_slot.intersect(ui.clip_rect()));
                ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
                let scroll_id = Id::new("settings_body_scroll");
                with_overlay_scroll(ui, scroll_id, |ui| {
                    // Parent owns body_h (plate geometry) — never available_height.
                    let scroll_h = scroll_slot.height().max(1.0);
                    let out = ScrollArea::vertical()
                        .id_salt("settings_body")
                        .max_height(scroll_h)
                        .min_scrolled_height(scroll_h)
                        .auto_shrink([false, false])
                        .show_viewport(ui, |ui, viewport| {
                            let content_min = ui.max_rect().min;
                            let view_screen = Rect::from_min_size(
                                content_min + viewport.min.to_vec2(),
                                viewport.size(),
                            );
                            // Intersect with inset slot (not full content_rect).
                            ui.set_clip_rect(view_screen.intersect(scroll_slot));
                            ui.set_min_width(content_w);
                            ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
                            crate::components::with_pad_fit(ui, Space::Lg, |ui| {
                                let heading = ui.label(
                                    TypeRole::Heading
                                        .rich(cat.title())
                                        .strong()
                                        .color(t.neutral_fg()),
                                );
                                readout.page_heading = Some(heading.rect);
                                ui.add(Spacer::new(Space::Md));
                                match cat {
                                    SettingsCat::Account => {
                                        super::settings_account::page_account(app, ui, t, queue)
                                    }
                                    SettingsCat::App => page_app(app, ui, t, queue),
                                    SettingsCat::Editor => page_editor(app, ui, t, queue),
                                    SettingsCat::Debug => page_debug(app, ui, t, queue),
                                }
                            });
                        });
                    ((), out.state.offset.y, out.id)
                });
            });

            // Hairlines last so scroll content cannot cover them (paint order).
            // Top: content column only (header↔rail stay continuous secondary).
            // Left: full content height at the rail join.
            let edge = Stroke::new(STROKE_HAIRLINE, t.neutral());
            let top_y = content_rect.top() + STROKE_HAIRLINE * 0.5;
            let left_x = content_rect.left() + STROKE_HAIRLINE * 0.5;
            ui.painter().hline(content_rect.x_range(), top_y, edge);
            ui.painter().vline(left_x, content_rect.y_range(), edge);
        });

    ctx.data_mut(|d| d.insert_temp(layout_readout_id(), readout));
}

/// Theme and window prefs.
fn page_app(app: &mut ShellApp, ui: &mut Ui, t: &Theme, queue: &mut Vec<Action>) {
    section_label(ui, t, "Appearance");
    form_group(ui, t, |ui| {
        let mut mode = match app.settings.theme_mode {
            crate::components::ModePreference::System => 0,
            crate::components::ModePreference::Light => 1,
            crate::components::ModePreference::Dark => 2,
        };
        if form_segmented(ui, t, "Mode", &["System", "Light", "Dark"], &mut mode).changed() {
            let pref = match mode {
                1 => crate::components::ModePreference::Light,
                2 => crate::components::ModePreference::Dark,
                _ => crate::components::ModePreference::System,
            };
            queue.push(A::SetThemeMode(pref));
        }
        let names: Vec<&str> = ThemeFamily::ALL.iter().map(|f| f.name()).collect();
        let mut fam = ThemeFamily::ALL
            .iter()
            .position(|f| f.name() == app.settings.theme_name)
            .unwrap_or(0)
            .min(names.len().saturating_sub(1));
        if form_picker(ui, t, "Theme", &names, &mut fam).changed() {
            queue.push(A::SetThemeFamily(ThemeFamily::ALL[fam]));
        }
    });

    ui.add(Spacer::new(Space::Lg));
    section_label(ui, t, "Tabs");
    form_group(ui, t, |ui| {
        let mut open_in_new_tab = app
            .session
            .ready()
            .map(|r| r.workspace.cfg.get_open_in_new_tab())
            .unwrap_or(true);
        if form_toggle_detail(
            ui,
            t,
            "Open files in new tabs",
            "When off, files open in the current tab by default.",
            &mut open_in_new_tab,
        )
        .changed()
        {
            queue.push(A::SetPrefOpenInNewTab(open_in_new_tab));
        }
    });

    ui.add(Spacer::new(Space::Lg));
    section_label(ui, t, "Window");
    form_group(ui, t, |ui| {
        let mut usage = app.settings.sidebar_usage;
        if form_toggle(ui, t, "Show usage in sidebar", &mut usage).changed() {
            queue.push(A::SetPrefSidebarUsage(usage));
        }
        #[cfg(target_os = "linux")]
        {
            let mut wayland = app.settings.allow_wayland;
            if form_toggle_detail(
                ui,
                t,
                "Allow Wayland",
                "Enables fractional scaling; disables drag-and-drop. Restart required.",
                &mut wayland,
            )
            .changed()
            {
                queue.push(A::SetPrefAllowWayland(wayland));
            }
        }
    });
}

/// Note-adjacent prefs — workspace.cfg only.
fn page_editor(app: &mut ShellApp, ui: &mut Ui, t: &Theme, queue: &mut Vec<Action>) {
    section_label(ui, t, "Links");
    form_group(ui, t, |ui| {
        let mut previews = app
            .session
            .ready()
            .map(|r| r.workspace.cfg.get_contact_linked_sites())
            .unwrap_or(false);
        if form_toggle_detail(
            ui,
            t,
            "Fetch link previews",
            "Contacting linked sites reveals your IP and that you opened the note.",
            &mut previews,
        )
        .changed()
        {
            queue.push(A::SetPrefLinkPreviews(previews));
        }
    });
}

fn page_debug(app: &ShellApp, ui: &mut Ui, t: &Theme, queue: &mut Vec<Action>) {
    // Integrity + panics load off-thread; Copy works without revealing the dump.
    let cache = app
        .debug_info
        .lock()
        .ok()
        .map(|g| g.clone())
        .unwrap_or_default();
    if matches!(cache, super::DebugInfoCache::Idle) {
        queue.push(A::EnsureDebugInfo);
    }
    // Silent load: no "Generating…" row — it appeared for one frame then vanished
    // when the worker finished, which read as a layout flicker on first open.
    // Copy stays disabled until Ready; Refresh keeps that disabled while Loading.
    let (dump, can_copy) = match &cache {
        super::DebugInfoCache::Idle | super::DebugInfoCache::Loading => (None, false),
        super::DebugInfoCache::Ready(s) => (Some(s.as_str()), !s.is_empty()),
    };

    // Snapshot first — short, always useful; dump is long (panics) and last.
    section_label(ui, t, "Build");
    form_group(ui, t, |ui| {
        form_value(ui, t, "Version", env!("CARGO_PKG_VERSION"));
        form_value(
            ui,
            t,
            "Platform",
            &format!(
                "{}.{}.{}",
                std::env::consts::ARCH,
                std::env::consts::FAMILY,
                std::env::consts::OS
            ),
        );
    });

    let snap = debug_snapshot(app);
    ui.add(Spacer::new(Space::Lg));
    section_label(ui, t, "Account snapshot");
    form_group(ui, t, |ui| {
        form_value(ui, t, "Username", &snap.username);
        form_value(ui, t, "Server", &snap.server);
        form_value(ui, t, "Data folder", &snap.data_dir);
    });

    ui.add(Spacer::new(Space::Lg));
    section_label(ui, t, "Support");
    form_group(ui, t, |ui| {
        // RTL trailing: first = rightmost → Copy (primary) · Refresh · Reveal.
        form_row(ui, t, "Debug info", |ui| {
            if Button::primary(t, "Copy")
                .enabled(can_copy)
                .copy_feedback("shell_copy_debug")
                .show(ui)
                .clicked()
            {
                queue.push(A::CopyDebugInfo);
            }
            ui.add(Spacer::new(Space::Sm).fill_cross(control_height()));
            if Button::quiet(t, "Refresh").show(ui).clicked() {
                queue.push(A::RefreshDebugInfo);
            }
            ui.add(Spacer::new(Space::Sm).fill_cross(control_height()));
            if app.debug_info_revealed {
                if Button::quiet(t, "Hide").show(ui).clicked() {
                    queue.push(A::HideDebugInfo);
                }
            } else if Button::quiet(t, "Reveal").show(ui).clicked() {
                queue.push(A::RevealDebugInfo);
            }
        });
    });

    if app.debug_info_revealed {
        if let Some(dump) = dump {
            ui.add(Spacer::new(Space::Sm));
            // Elevated surface + Outside hairline. Natural height (newlines);
            // horizontal scroll for long lines — outer settings body scrolls
            // vertically (no nested vertical / fixed 320 box that overflowed).
            crate::components::plate_content(
                ui,
                t.neutral_bg_secondary(),
                t.neutral(),
                Radius::Control.corner(),
                |ui| {
                    ui.set_width(ui_width(ui).max(1.0));
                    ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
                    crate::components::with_pad_fit(ui, Space::Sm, |ui| {
                        let mid_w = crate::components::ui_width(ui).max(1.0);
                        ScrollArea::horizontal()
                            .id_salt("settings_debug_dump")
                            .max_width(mid_w)
                            .auto_shrink([false, true])
                            .show(ui, |ui| {
                                ui.add(
                                    Label::new(TypeRole::Mono.rich(dump).color(t.neutral_fg()))
                                        .wrap_mode(TextWrapMode::Extend),
                                );
                            });
                    });
                },
            );
        }
    }
}

struct DebugSnapshot {
    username: String,
    server: String,
    data_dir: String,
}

fn debug_snapshot(app: &ShellApp) -> DebugSnapshot {
    let Some(r) = app.session.ready() else {
        return DebugSnapshot {
            username: "—".into(), server: "—".into(), data_dir: "—".into()
        };
    };

    let account = r.workspace.core.get_account().ok();
    let username = account
        .as_ref()
        .map(|a| a.username.clone())
        .unwrap_or_else(|| r.workspace.account.username.clone());
    let server = account
        .as_ref()
        .map(|a| a.api_url.clone())
        .unwrap_or_else(|| "—".into());
    let data_dir = r.workspace.core.get_config().writeable_path.clone();

    DebugSnapshot { username, server, data_dir }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::{ThemeExt, install};
    use crate::shell::ShellApp;
    use egui::{Context, FullOutput, Pos2, RawInput, Vec2};

    fn layout_settings_at(screen_w: f32, screen_h: f32, cat: SettingsCat) -> SettingsLayoutReadout {
        let mut app = ShellApp { modal: Some(Modal::Settings { cat }), ..Default::default() };
        let ctx = Context::default();
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(screen_w, screen_h))),
            ..Default::default()
        };
        // Fonts bind after the frame that calls set_fonts (Phosphor nav icons).
        let FullOutput { .. } = ctx.run(input.clone(), |ctx| {
            install(ctx);
        });
        let FullOutput { .. } = ctx.run(input, |ctx| {
            install(ctx);
            let t = ctx.get_lb_theme();
            let mut queue = Vec::new();
            show(&mut app, ctx, &t, &mut queue);
        });
        take_layout_readout(&ctx).expect("settings should publish layout readout")
    }

    fn assert_plate_sane(r: &SettingsLayoutReadout, screen_h: f32) {
        assert!(
            r.plate.width() > 1.0 && r.plate.height() > 1.0,
            "plate collapsed at screen_h={screen_h}: {:?}",
            r.plate
        );
        assert!(
            r.plate.height() <= r.screen.height() + 0.5,
            "plate taller than screen at h={screen_h}: plate={} screen={}",
            r.plate.height(),
            r.screen.height()
        );
        assert!(
            (r.plate.height() - (r.header.height() + r.body.height())).abs() < 1.0,
            "header+body must fill plate at screen_h={screen_h}: plate={} header={} body={}",
            r.plate.height(),
            r.header.height(),
            r.body.height()
        );
        assert!(
            (r.body.height() - r.content.height()).abs() < 0.5,
            "content canvas must match body height (no receding fill) at screen_h={screen_h}: body={} content={}",
            r.body.height(),
            r.content.height()
        );
        assert!(
            (r.content.bottom() - r.body.bottom()).abs() < 0.5,
            "content bottom must meet body bottom at screen_h={screen_h}: content.b={} body.b={}",
            r.content.bottom(),
            r.body.bottom()
        );
        assert!(
            (r.rail.height() - r.body.height()).abs() < 0.5,
            "rail must match body height at screen_h={screen_h}"
        );
        // Something interactive must still be laid out (not a blank plate).
        let heading = r
            .page_heading
            .expect("page heading should allocate at screen_h={screen_h}");
        assert!(
            heading.width() > 1.0 && heading.height() > 1.0,
            "page heading empty at screen_h={screen_h}: {heading:?}"
        );
        // Heading lives in the content column (may scroll, but origin is in plate).
        assert!(
            heading.min.x >= r.content.min.x - 1.0,
            "heading left of content plate at screen_h={screen_h}"
        );
        let nav = r
            .first_nav
            .expect("first nav item should allocate at screen_h={screen_h}");
        assert!(
            nav.width() > 1.0 && nav.height() > 1.0,
            "nav empty at screen_h={screen_h}: {nav:?}"
        );
    }

    #[test]
    fn plate_origin_centers_when_room() {
        let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(1300.0, 900.0));
        let (w, h) = plate_size_for_screen(screen);
        let o = plate_origin_for_screen(screen, w, h);
        let plate = Rect::from_min_size(o, Vec2::new(w, h));
        // True vertical center when the plate fits below the top safe inset.
        assert!(
            (plate.center().y - screen.center().y).abs() < 0.5,
            "expected vertical center, plate.center.y={} screen.center.y={}",
            plate.center().y,
            screen.center().y
        );
        assert!((plate.center().x - screen.center().x).abs() < 0.5, "expected horizontal center");
        // On a tall screen the plate sits below y=0; never above the viewport.
        assert!(plate.top() >= screen.top() - 0.5, "top={}", plate.top());
    }

    #[test]
    fn plate_origin_clamps_top_when_center_would_hit_safe() {
        // Short window: centered plate would violate the top safe inset.
        let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(900.0, 560.0));
        let (w, h) = plate_size_for_screen(screen);
        let o = plate_origin_for_screen(screen, w, h);
        let plate = Rect::from_min_size(o, Vec2::new(w, h));
        // Clamped origin is never above the screen top.
        assert!(plate.top() + 0.5 >= screen.top(), "top {} above screen", plate.top());
        // Prefer center when possible; when clamped, top is as close to center as allowed.
        let ideal_top = screen.center().y - h * 0.5;
        assert!(plate.top() + 0.5 >= ideal_top.min(screen.top()), "unexpected top={}", plate.top());
    }

    #[test]
    fn settings_plate_fills_at_default_height() {
        let r = layout_settings_at(1300.0, 800.0, SettingsCat::Account);
        assert_plate_sane(&r, 800.0);
        let (_, want_h) =
            plate_size_for_screen(Rect::from_min_size(Pos2::ZERO, Vec2::new(1300.0, 800.0)));
        assert!(
            (r.plate.height() - want_h).abs() < 1.0,
            "expected full preferred height {want_h}, got {}",
            r.plate.height()
        );
        // Live layout must match our math (not Area CENTER + half-offset).
        let want = plate_origin_for_screen(r.screen, r.plate.width(), r.plate.height());
        assert!(
            (r.plate.min.x - want.x).abs() < 1.0 && (r.plate.min.y - want.y).abs() < 1.0,
            "plate origin {:?} != expected {want:?}",
            r.plate.min
        );
    }

    /// Shrink the window from the bottom: content canvas must stay glued to the
    /// plate bottom (the “receding frame” bug).
    #[test]
    fn settings_content_tracks_body_as_height_shrinks() {
        // From comfortable down through “just past last rail tab” to cramped.
        let heights =
            [800.0, 600.0, 520.0, 480.0, 420.0, 360.0, 300.0, 260.0, 220.0, 180.0, 140.0, 100.0];
        for h in heights {
            let r = layout_settings_at(900.0, h, SettingsCat::Account);
            assert_plate_sane(&r, h);
            let (want_w, want_h) =
                plate_size_for_screen(Rect::from_min_size(Pos2::ZERO, Vec2::new(900.0, h)));
            assert!(
                (r.plate.height() - want_h).abs() < 1.0,
                "plate height {} != expected {want_h} at screen_h={h}",
                r.plate.height()
            );
            assert!(
                (r.plate.width() - want_w).abs() < 1.0,
                "plate width {} != expected {want_w} at screen_h={h}",
                r.plate.width()
            );
            // Body must keep a usable share of the plate (not stolen by header pad).
            assert!(
                r.body.height() >= r.header.height() * 0.5 || r.plate.height() < 80.0,
                "body starved by header at screen_h={h}: header={} body={}",
                r.header.height(),
                r.body.height()
            );
        }
    }

    #[test]
    fn settings_layout_all_cats_at_cramped_height() {
        for cat in SettingsCat::ALL {
            let r = layout_settings_at(800.0, 280.0, cat);
            assert_plate_sane(&r, 280.0);
        }
    }

    #[test]
    fn settings_does_not_panic_at_tiny_screen() {
        for h in [80.0, 60.0, 40.0, 24.0] {
            let r = layout_settings_at(400.0, h, SettingsCat::App);
            assert!(r.plate.height() >= 1.0 && r.plate.width() >= 1.0, "collapsed plate at h={h}");
            // Geometry still partitions even when cramped.
            assert!(
                (r.plate.height() - (r.header.height() + r.body.height())).abs() < 1.5,
                "partition broken at h={h}"
            );
        }
    }
}
