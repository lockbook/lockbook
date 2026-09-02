//! Multi-atom patterns: form, sheet chrome, lists, scroll, menus, tips.

pub mod context_menu;
pub mod form;
pub mod list_chrome;
pub mod scroll;
pub mod sheet;
pub mod tip;
pub use form::{
    ack_row, footnote, form_group, form_picker, form_row, form_row_detail, form_segmented,
    form_toggle, form_toggle_detail, form_value, section_label,
};
pub use list_chrome::{LIST_PAD, SECTION_GAP, SECTION_HEAD_GAP, paint_list_section};
pub use scroll::{fixed_height_list, with_overlay_scroll};
pub use sheet::{
    SheetFooterOpts, sheet_band, sheet_band_centered, sheet_dim, sheet_equal_row, sheet_footer,
    sheet_panel_fit, sheet_panel_fixed, sheet_title_muted,
};
pub use tip::{tip_card_placed, tip_text};
