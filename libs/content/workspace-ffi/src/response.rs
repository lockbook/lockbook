use egui::{Context, CursorIcon, PlatformOutput, ViewportCommand, ViewportIdMap, ViewportOutput};
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
    pub request_paste: bool,
    pub context_menu: Option<egui::Pos2>,
}

impl Response {
    pub fn new(
        context: &Context, platform: PlatformOutput, viewport: ViewportIdMap<ViewportOutput>,
        workspace: workspace_rs::Response,
    ) -> Self {
        let mut redraw_in = None;
        let mut window_title = None;
        let mut request_paste = false;
        if let Some(viewport) = viewport.into_values().next() {
            redraw_in = Some(viewport.repaint_delay.as_millis() as _);
            for cmd in viewport.commands.into_iter() {
                match cmd {
                    ViewportCommand::Title(title) => window_title = Some(title),
                    ViewportCommand::RequestPaste => request_paste = true,
                    _ => {} // remaining viewport commands ignored (many such cases!)
                }
            }
        } else {
            eprintln!("viewport missing: not redrawing or setting window title");
        }

        Self {
            workspace,
            redraw_in,
            copied_text: platform.copied_text,
            url_opened: platform.open_url.map(|u| u.url), // todo: expose "new_tab" field
            cursor: platform.cursor_icon,
            virtual_keyboard_shown: context.pop_virtual_keyboard_shown(),
            window_title,
            request_paste,
            context_menu: context.pop_context_menu(),
        }
    }
}
