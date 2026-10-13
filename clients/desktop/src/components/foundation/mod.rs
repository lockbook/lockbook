//! Theme, layout, interact, chrome metrics — no product surfaces.
//!
//! Tokens and layout live in [`workspace_rs::style`]; this module re-exports
//! them and keeps desktop-only install ([`style`]).

pub use workspace_rs::style::{chrome, color, interact, layout, space, spacer, typography};
#[allow(unused_imports)]
pub use workspace_rs::style::{overlay, tree_metrics};

pub mod style;

pub use style::{ModePreference, ThemeFamily, install, set_mode_preference, set_theme_family};
pub use workspace_rs::style::ROW_H;
pub use workspace_rs::style::Space;
pub use workspace_rs::style::Spacer;
pub use workspace_rs::style::TypeRole;
pub use workspace_rs::style::handle_toggle_shortcut;
pub use workspace_rs::style::sense_click;
pub use workspace_rs::style::{FG_HOVER, FG_PRESS, Theme, ThemeExt};
pub use workspace_rs::style::{
    FixedPadContent, claim, origin, place_at, remaining_height, ui_width, with_h_pad,
    with_h_pad_in, with_pad_fit,
};
pub use workspace_rs::style::{
    Radius, STROKE_HAIRLINE, control_height, display_file_name, file_row_icon, paint_plate,
    paint_plate_stroke, phosphor, phosphor_font_id, phosphor_ui_font_id, pin_shadow, plate_content,
    shortcut_cmd_i, shortcut_cmd_n, shortcut_enter, shortcut_esc, shortcut_return, tab_icon,
};
#[cfg(test)]
pub use workspace_rs::style::{begin_spacer_record, take_spacer_record};
