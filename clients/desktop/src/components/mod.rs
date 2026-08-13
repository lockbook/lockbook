//! Lockbook desktop component library.

pub mod atoms;
pub mod foundation;

pub use foundation::{FixedPadContent, ModePreference, ROW_H, Radius, STROKE_HAIRLINE, Space, Spacer, ThemeFamily, Theme, TypeRole, claim, control_height, file_row_icon, fit_outside_stroke_fill, handle_toggle_shortcut, install, origin, paint_plate, paint_plate_stroke, phosphor, phosphor_ui_font_id, place_at, plate_content, remaining_height, sense_click, set_mode_preference, set_theme_family, shortcut_cmd_i, shortcut_cmd_n, shortcut_enter, shortcut_esc, shortcut_return, ui_width, with_h_pad, with_h_pad_in, with_pad_fit};
#[cfg(test)]
pub use foundation::{begin_spacer_record, take_spacer_record};

pub use atoms::{Button, Field, icon_button, icon_button_hit};

pub mod interact {
    pub use super::foundation::interact::*;
}
