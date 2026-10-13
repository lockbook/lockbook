//! Space debug overlay. F2 toggles; when on, [`super::Spacer`] paints token colors.

use egui::{Context, Id, Key};

const ID: &str = "lb.design.space_overlay";

fn id() -> Id {
    Id::new(ID)
}

pub fn is_enabled(ctx: &Context) -> bool {
    ctx.data(|d| d.get_temp::<bool>(id()).unwrap_or(false))
}

pub fn set_enabled(ctx: &Context, on: bool) {
    ctx.data_mut(|d| d.insert_temp(id(), on));
}

pub fn toggle(ctx: &Context) {
    let next = !is_enabled(ctx);
    set_enabled(ctx, next);
}

/// Call once per frame from product chrome.
pub fn handle_toggle_shortcut(ctx: &Context) {
    if ctx.input(|i| i.key_pressed(Key::F2)) {
        toggle(ctx);
    }
}
