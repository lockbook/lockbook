use egui::{Context, CursorIcon, PlatformOutput, ViewportIdMap, ViewportOutput};
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
    pub copied_text: String,
    pub url_opened: Option<String>,
    pub cursor: CursorIcon,
    pub virtual_keyboard_shown: Option<bool>,
    pub window_title: Option<String>,
    pub context_menu: Option<egui::Pos2>,
}

impl Response {
    pub fn new(
        context: &Context, platform: PlatformOutput, viewport: ViewportIdMap<ViewportOutput>,
        workspace: workspace_rs::Response,
    ) -> Self {
        let redraw_in = match viewport.values().next().map(|v| v.repaint_delay) {
            Some(d) => Some(d.as_millis() as u64),
            None => {
                eprintln!("VIEWPORT Missing, not requesting redraw");
                None
            }
        };

        Self {
            workspace,
            redraw_in,
            copied_text: platform.copied_text,
            url_opened: platform.open_url.map(|u| u.url), // todo: expose "new_tab" field
            cursor: platform.cursor_icon,
            virtual_keyboard_shown: context.pop_virtual_keyboard_shown(),
            window_title: context.pop_window_title(),
            context_menu: context.pop_context_menu(),
        }
    }
}
