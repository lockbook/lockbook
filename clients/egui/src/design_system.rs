//! The design-system page — a living spec that doubles as a theme editor. It
//! renders the tokens and text roles at spec so their values can be dialed in
//! against the real renderer. Sections: typography, buttons; color/surfaces next.

use egui::{Align, Context, Layout, Sense, Stroke, Ui, vec2};
use workspace_rs::theme::palette_v2::{Mode, Theme, ThemeExt};

use crate::theme::text;
use crate::theme::tokens::Tokens;
use crate::widgets::button::Button;

pub fn show(ctx: &Context, ui: &mut Ui, t: &Tokens, mode: &mut Mode) {
    // Two egui layout rules worth keeping in mind everywhere: `with_layout` grabs
    // all remaining cross-axis space (pass an explicit desired size to keep a row
    // content-sized), and a row is measured as it goes — an item can only align
    // against siblings placed *before* it. So the height-defining item (the
    // toggle) goes first, then the heading bottom-aligns against it.
    let next = if *mode == Mode::Dark { "Light" } else { "Dark" };
    let toggled = ui
        .allocate_ui_with_layout(
            vec2(ui.available_width(), ui.spacing().interact_size.y),
            Layout::right_to_left(Align::Max),
            |ui| {
                let clicked = Button::secondary(t, next).show(ui).clicked();
                ui.with_layout(Layout::left_to_right(Align::Max), |ui| {
                    ui.label(text::heading(t, "Design System"));
                });
                clicked
            },
        )
        .inner;
    if toggled {
        *mode = if *mode == Mode::Dark { Mode::Light } else { Mode::Dark };
        ctx.set_lb_theme(Theme::default(*mode));
        ctx.request_repaint();
    }
    ui.add_space(8.0);
    rule(ui, t);

    section(ui, t, "Typography");
    typography(ui, t);

    section(ui, t, "Buttons");
    buttons(ui, t);
}

/// The three roles, each specimen rendering its own name so the sample and the
/// spec are one and the same.
fn typography(ui: &mut Ui, t: &Tokens) {
    ui.label(text::heading(t, "Heading — sans 18"));
    ui.add_space(12.0);
    ui.label(text::text(t, "Text — sans 14, the body of everything"));
    ui.add_space(12.0);
    ui.label(text::mono(t, "Mono — mono 14, for data and code"));
}

/// Treatments across a row, plus tone/state modifiers. Interact (hover, press,
/// Tab-to-focus) to see the states — they're color shifts, no lift.
fn buttons(ui: &mut Ui, t: &Tokens) {
    ui.horizontal(|ui| {
        Button::primary(t, "Primary").show(ui);
        Button::secondary(t, "Secondary").show(ui);
        Button::quiet(t, "Quiet").show(ui);
        Button::secondary(t, "Delete").danger().show(ui);
    });
    ui.add_space(10.0);
    ui.label(text::mono(t, "hover · press · Tab-focus to see states").color(t.text_faint()));
}

/// A minor section header on the page — mono, uppercased, faint.
fn section(ui: &mut Ui, t: &Tokens, title: &str) {
    ui.add_space(26.0);
    ui.label(text::mono(t, title.to_uppercase()).color(t.text_faint()));
    ui.add_space(12.0);
}

fn rule(ui: &mut Ui, t: &Tokens) {
    let w = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(w, 1.0), Sense::hover());
    ui.painter()
        .hline(rect.x_range(), rect.center().y, Stroke::new(1.0, t.line()));
}
