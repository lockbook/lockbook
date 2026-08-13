//! Theme, layout, interact, chrome metrics — no product surfaces.

pub mod chrome;
pub mod color;
pub mod interact;
pub mod layout;
pub mod overlay;
pub mod space;
pub mod spacer;
pub mod style;
pub mod tree_metrics;
pub mod typography;

pub use chrome::{
    Radius, STROKE_HAIRLINE, control_height, file_row_icon, fit_outside_stroke_fill, paint_plate,
    paint_plate_stroke, phosphor, phosphor_ui_font_id, plate_content, shortcut_cmd_i,
    shortcut_cmd_n, shortcut_enter, shortcut_esc, shortcut_return,
};
pub use color::Theme;
pub use interact::sense_click;
pub use layout::{
    FixedPadContent, claim, origin, place_at, remaining_height, ui_width, with_h_pad,
    with_h_pad_in, with_pad_fit,
};
pub use overlay::handle_toggle_shortcut;
pub use space::Space;
pub use spacer::Spacer;
#[cfg(test)]
pub use spacer::{begin_record as begin_spacer_record, take_record as take_spacer_record};
pub use style::{ModePreference, ThemeFamily, install, set_mode_preference, set_theme_family};
pub use tree_metrics::ROW_H;
pub use typography::TypeRole;
