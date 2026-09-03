//! Shared design system: space, color, type, layout, and controls.
//!
//! Desktop re-exports this crate as `components::{foundation, atoms}` so the
//! shell and workspace chrome (landing, search) paint from one set of tokens.

pub mod button;
pub mod chrome;
pub mod color;
pub mod context_menu;
pub mod field;
pub mod file_name;
pub mod file_row;
pub mod interact;
pub mod layout;
pub mod list_chrome;
pub mod motion;
pub mod overlay;
pub mod segmented;
pub mod space;
pub mod spacer;
pub mod tree_metrics;
pub mod typography;

pub use chrome::{
    Radius, STROKE_HAIRLINE, control_height, display_file_name, file_row_icon,
    fit_outside_stroke_fill, paint_plate, paint_plate_stroke, phosphor, phosphor_font_id,
    phosphor_ui_font_id, plate_content, shortcut_cmd_i, shortcut_cmd_n, shortcut_cmd_o,
    shortcut_enter, shortcut_esc, shortcut_return, tab_icon,
};
pub use color::{FG_HOVER, FG_PRESS, Theme, ThemeExt};
pub use interact::sense_click;
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
pub use segmented::{segmented, segmented_h, segmented_width};

pub use list_chrome::{LIST_PAD, SECTION_GAP, SECTION_HEAD_GAP, paint_list_section};
pub use motion::{SURFACE_SECS, SurfaceMotion, surface_motion};
