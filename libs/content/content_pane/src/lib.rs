use lb_editor::EditorResponse;

pub enum ContentPane {
    Markdown(lb_editor::Editor),
    Drawing,
}

pub struct ContentPaneResp {
    content_changed: bool,

    integration: IntegrationResp,
    toolbar: ToolbarResp,
}

pub struct ToolbarResp {
    pub cursor_in_heading: bool,
    pub cursor_in_bullet_list: bool,
    pub cursor_in_number_list: bool,
    pub cursor_in_todo_list: bool,
    pub cursor_in_bold: bool,
    pub cursor_in_italic: bool,
    pub cursor_in_inline_code: bool,
}

pub struct IntegrationResp {
    pub show_edit_menu: bool,
    pub has_selection: bool,
    pub edit_menu_x: f32,
    pub edit_menu_y: f32,
}

impl ContentPane {
    pub fn new(title: &str, content: Vec<u8>) -> Self {
        if title.ends_with(".md") {
            Self::Markdown(lb_editor::Editor::new(&String::from_utf8(content).unwrap()))
        } else {
            Self::Drawing
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) -> ContentPaneResp {
        match self {
            ContentPane::Markdown(editor) => editor.scroll_ui(ui).into(),
            ContentPane::Drawing => todo!(),
        }
    }

    pub fn current_content(&self) -> Vec<u8> {
        match self {
            ContentPane::Markdown(editor) => editor.buffer.current.text.clone().into_bytes(),
            ContentPane::Drawing => todo!(),
        }
    }
}

impl From<EditorResponse> for ContentPaneResp {
    fn from(value: EditorResponse) -> Self {
        Self {
            content_changed: value.text_updated,
            integration: IntegrationResp {
                show_edit_menu: value.show_edit_menu,
                has_selection: value.has_selection,
                edit_menu_x: value.edit_menu_x,
                edit_menu_y: value.edit_menu_y,
            },
            toolbar: ToolbarResp {
                cursor_in_heading: value.cursor_in_heading,
                cursor_in_bullet_list: value.cursor_in_bullet_list,
                cursor_in_number_list: value.cursor_in_number_list,
                cursor_in_todo_list: value.cursor_in_todo_list,
                cursor_in_bold: value.cursor_in_bold,
                cursor_in_italic: value.cursor_in_italic,
                cursor_in_inline_code: value.cursor_in_inline_code,
            },
        }
    }
}
