use egui::{Context, CursorIcon, FullOutput};
use workspace_rs::tab::ExtendedOutput;

// This general purpose workspace-ffi response captures the workspace widget response and platform response, but each
// platform translates the workspace response into its own platform-specific response with just those fields that make
// sense on that platform.
#[derive(Debug, Default)]
pub struct Response {
    // widget response
    pub workspace: workspace_rs::Response,

    // platform response
    pub redraw_in: Option<u64>,
    pub copied_text: String, // unlike other fields, not optional (based on egui data model)
    pub url_opened: Option<String>,
    pub cursor: Option<CursorIcon>,
    pub virtual_keyboard_shown: Option<bool>,
    pub window_title: Option<String>,
    pub context_menu: Option<egui::Pos2>,
}

impl Response {
    pub fn new(
        context: Context, full_output: FullOutput, workspace: workspace_rs::Response,
    ) -> Self {
        let redraw_in = match full_output
            .viewport_output
            .values()
            .next()
            .map(|v| v.repaint_delay)
        {
            Some(d) => Some(d.as_millis() as u64),
            None => {
                eprintln!("VIEWPORT Missing, not requesting redraw");
                None
            }
        };

        Self {
            workspace,
            redraw_in,
            copied_text: full_output.platform_output.copied_text,
            url_opened: full_output.platform_output.open_url.map(|u| u.url), // todo: expose "new_tab" field
            cursor: full_output.platform_output.cursor_icon.into(),
            virtual_keyboard_shown: context.pop_virtual_keyboard_shown(),
            window_title: context.pop_window_title(),
            context_menu: context.pop_context_menu(),
        }
    }
}
