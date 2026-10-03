use lb_c::Uuid;
use lb_c::model::text::offset_types::{Grapheme, Graphemes};
use serde::Serialize;
use workspace_rs::tab::markdown_editor::input::{Location, Region};

#[derive(Serialize)]
pub struct AndroidResponse {
    // platform response
    pub redraw_in: u64,
    pub copied_text: String,
    pub copied_html: String,
    pub copied_image: Vec<u8>,
    pub has_url_opened: bool,
    pub url_opened: String,
    pub virtual_keyboard_shown: Option<bool>,

    // widget response
    pub selected_file: Uuid,
    pub doc_created: Uuid,

    pub tabs_changed: bool,

    pub has_edit_menu: bool,
    pub edit_menu_x: f32,
    pub edit_menu_y: f32,
    /// The menu is over a selected atom (image, link card/capsule) — offer "Edit"
    /// (`enterSelectedAtom`) alongside the standard actions.
    pub edit_menu_for_atom: bool,
    /// The menu is over an image viewer — offer "Copy image".
    pub edit_menu_for_image: bool,

    pub selection_updated: bool,
    pub text_updated: bool,
    pub failure_message: String,
    pub scroll_updated: bool,

    pub has_text_interaction_rect: bool,
    pub text_interaction_min_x: f32,
    pub text_interaction_min_y: f32,
    pub text_interaction_max_x: f32,
    pub text_interaction_max_y: f32,

    /// Handle-drag magnifier source (egui points, workspace space).
    pub has_magnifier: bool,
    pub magnifier_x: f32,
    pub magnifier_y: f32,

    /// A text field of the workspace's own chrome has focus, so the
    /// keyboard's text goes to it as egui events.
    pub chrome_text_focused: bool,
}

impl From<crate::Response> for AndroidResponse {
    fn from(value: crate::Response) -> Self {
        let crate::Response {
            workspace:
                workspace_rs::Response {
                    selected_file,
                    selected_session: _,
                    file_renamed: _,
                    file_moved: _,
                    file_deleted: _,
                    new_folder_clicked: _,
                    tab_title_clicked: _,
                    file_created,
                    markdown_editor_text_updated,
                    markdown_editor_selection_updated,
                    markdown_editor_scroll_updated,
                    text_interaction_rect,
                    mobile_toolbar_shown: _,
                    caret_covered: _,
                    tabs_changed,
                    failure_messages,
                    selected_folder_changed: _,
                    open_camera: _,
                    file_cache_updated: _,
                    open_search: _,
                },
            redraw_in,
            copied_text,
            copied_html,
            copied_image,
            urls_opened,
            cursor: _,
            virtual_keyboard_shown,
            window_title: _,
            request_paste: _,
            ime_cursor: _,
            context_menu,
        } = value;

        let doc_created = match file_created {
            Some(Ok(ref f)) if f.is_document() => f.id,
            _ => Uuid::nil(),
        };
        Self {
            failure_message: failure_messages.join("\n"),
            selected_file: selected_file.unwrap_or_default(),
            doc_created,
            tabs_changed,
            redraw_in: redraw_in.unwrap_or(u64::MAX),
            copied_text,
            copied_html,
            copied_image,
            has_url_opened: !urls_opened.is_empty(),
            url_opened: urls_opened.into_iter().next().unwrap_or_default(),
            text_updated: markdown_editor_text_updated,
            selection_updated: markdown_editor_selection_updated,
            scroll_updated: markdown_editor_scroll_updated,
            has_text_interaction_rect: text_interaction_rect.is_some(),
            text_interaction_min_x: text_interaction_rect.map(|r| r.min.x).unwrap_or_default(),
            text_interaction_min_y: text_interaction_rect.map(|r| r.min.y).unwrap_or_default(),
            text_interaction_max_x: text_interaction_rect.map(|r| r.max.x).unwrap_or_default(),
            text_interaction_max_y: text_interaction_rect.map(|r| r.max.y).unwrap_or_default(),
            has_magnifier: false,
            magnifier_x: 0.0,
            magnifier_y: 0.0,
            has_edit_menu: context_menu.is_some(),
            edit_menu_x: context_menu.unwrap_or_default().0.x,
            edit_menu_y: context_menu.unwrap_or_default().0.y,
            edit_menu_for_atom: matches!(
                context_menu,
                Some((_, workspace_rs::tab::ContextMenuTarget::Atom))
            ),
            edit_menu_for_image: matches!(
                context_menu,
                Some((_, workspace_rs::tab::ContextMenuTarget::Image))
            ),
            virtual_keyboard_shown,
            chrome_text_focused: false,
        }
    }
}

#[derive(Serialize, Default)]
pub struct JTextPosition {
    pub none: bool,
    pub position: usize,
}

#[derive(Serialize, Default)]
pub struct JTextRange {
    pub none: bool,
    pub start: usize,
    pub end: usize,
}

#[derive(Serialize, Default)]
pub struct JRect {
    pub none: bool,
    pub min_x: f32,
    pub min_y: f32,
    pub max_x: f32,
    pub max_y: f32,
}

impl JRect {
    pub fn from_line(line: [egui::Pos2; 2]) -> Self {
        Self { none: false, min_x: line[0].x, min_y: line[0].y, max_x: line[1].x, max_y: line[1].y }
    }
}

impl From<JTextRange> for (Grapheme, Grapheme) {
    fn from(value: JTextRange) -> Self {
        (value.start.into(), value.end.into())
    }
}

impl From<JTextRange> for (Graphemes, Graphemes) {
    fn from(value: JTextRange) -> Self {
        (value.start.into(), value.end.into())
    }
}

impl From<JTextRange> for Region {
    fn from(value: JTextRange) -> Self {
        Region::BetweenLocations {
            start: Location::Grapheme(value.start.into()),
            end: Location::Grapheme(value.start.into()),
        }
    }
}
