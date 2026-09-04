//! Lockbook desktop **component library**.
//!
//! Layers:
//! - [`foundation`] — space, color, type, layout (measure+place), interact, chrome
//! - [`atoms`] — button, chip, field, file row, nav, person, …
//! - [`compounds`] — form, sheet, list, scroll, menu, tips
//! - [`domain`] — app-shaped multi-caller surfaces (sticky tree, pins, tabs, …)
//!
//! Prefer `crate::components::…` (or re-exports below) over reaching into
//! submodules unless needed.

pub mod atoms;
pub mod compounds;
pub mod domain;
pub mod foundation;

pub use foundation::{
    FG_HOVER, FG_PRESS, FixedPadContent, ModePreference, ROW_H, Radius, STROKE_HAIRLINE, Space,
    Spacer, Theme, ThemeExt, ThemeFamily, TypeRole, claim, control_height, display_file_name,
    file_row_icon, fit_outside_stroke_fill, handle_toggle_shortcut, install, origin, paint_plate,
    paint_plate_stroke, phosphor, phosphor_font_id, phosphor_ui_font_id, place_at, plate_content,
    remaining_height, sense_click, set_mode_preference, set_theme_family, shortcut_cmd_i,
    shortcut_cmd_n, shortcut_enter, shortcut_esc, shortcut_return, tab_icon, ui_width, with_h_pad,
    with_h_pad_in, with_pad_fit,
};
#[cfg(test)]
pub use foundation::{begin_spacer_record, take_spacer_record};

pub use atoms::{
    Button, Chip, ChipHue, EqualCells, Field, FileRow, NavItem, PersonRow, PersonTone,
    QuietChipAlign, QuietChipLabel, icon_button, icon_button_glyph, icon_button_hit,
    measure_file_name, paint_file_name, person_row_height, quiet_chip, quiet_chip_height,
    quiet_chip_labeled_min_width, segmented, segmented_h,
};

pub use compounds::{
    LIST_PAD, SECTION_GAP, SECTION_HEAD_GAP, SheetFooterOpts, ack_row, fixed_height_list, footnote,
    form_group, form_picker, form_row, form_row_detail, form_segmented, form_toggle,
    form_toggle_detail, form_value, paint_list_section, section_label, sheet_band,
    sheet_band_centered, sheet_dim, sheet_equal_row, sheet_footer, sheet_panel_fit,
    sheet_panel_fixed, sheet_title_muted, tip_card_placed, tip_text, with_overlay_scroll,
};
pub use domain::{show_recents, show_shared, show_sync_footer, show_tree};

pub mod interact {
    pub use super::foundation::interact::*;
}
pub mod context_menu {
    pub use super::compounds::context_menu::*;
}
