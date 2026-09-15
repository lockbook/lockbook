//! Shared design system: space, color, type, layout, and controls.
//!
//! Desktop re-exports this crate as `components::{foundation, atoms}` so the
//! shell and workspace chrome (landing, editors) paint from one set of tokens.

pub mod button;
pub mod chrome;
pub mod color;
pub mod context_menu;
pub mod field;
pub mod file_name;
pub mod file_row;
pub mod interact;
pub mod layout;
pub mod overlay;
pub mod overlay_scroll;
pub mod space;
pub mod spacer;
pub mod tip;
pub mod tree_metrics;
pub mod typography;

pub use chrome::{
    ANIM_MAX_SECS, CHROME_BAND_GLYPH, CHROME_BAND_H, HOVER_ANIM_SECS, Radius, STROKE_HAIRLINE,
    TOGGLE_ANIM_SECS, canvas_overlay_frame, control_height, control_icon_hit, display_file_name,
    file_row_icon, fit_outside_stroke_fill, overlay_shadow, paint_plate, paint_plate_stroke,
    phosphor, phosphor_font_id, phosphor_ui_font_id, pin_shadow, plate_content, shortcut_cmd_i,
    shortcut_cmd_n, shortcut_cmd_o, shortcut_enter, shortcut_esc, shortcut_return, tab_icon,
};
pub use color::{FG_HOVER, FG_PRESS, Theme, ThemeExt};
pub use file_row::{ellipsize_path, parent_crumbs, path_crumbs};
pub use interact::{
    canvas_selected_fills, interact_fill, interact_fill_response, quiet_canvas_fills,
    quiet_secondary_fills, sense_click,
};
pub use layout::{
    FixedPadContent, claim, origin, place_at, remaining_height, ui_width, with_h_pad,
    with_h_pad_in, with_pad_fit,
};
pub use overlay::handle_toggle_shortcut;
pub use space::Space;
pub use spacer::Spacer;
pub use spacer::{begin_record as begin_spacer_record, take_record as take_spacer_record};
pub use tree_metrics::ROW_H;
pub use typography::TypeRole;

pub use button::{Button, icon_button, icon_button_glyph, icon_button_hit};
pub use field::Field;
pub use file_name::{measure as measure_file_name, paint_body as paint_file_name};
pub use file_row::FileRow;
pub use overlay_scroll::{SIDEBAR_RESIZING_LATCH, with_overlay_scroll};
pub use tip::{tip_card_placed, tip_text};
