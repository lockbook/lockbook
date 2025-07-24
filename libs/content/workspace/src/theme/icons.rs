use egui::TextWrapMode;

#[derive(Clone, PartialEq)]
pub struct Icon {
    pub has_badge: bool,
    pub icon: &'static str,
    pub size: f32,
    color: Option<egui::Color32>,
    weak: bool,
}

const fn ic(c: &'static str) -> Icon {
    Icon { has_badge: false, icon: c, size: 20.0, color: None, weak: false }
}

impl Icon {
    pub const ACCOUNT: Self = ic("\u{e7ff}"); // Person Outline
    pub const ARROW_CIRCLE_DOWN: Self = ic("\u{f181}"); // Arrow Circle Down
    pub const ARROW_DOWN: Self = ic("\u{e5c5}"); // Arrow Down
    pub const ARROW_UP: Self = ic("\u{e5c7}"); // Arrow Up
    pub const ARROW_BACK: Self = ic("\u{e5c4}");
    pub const ARROW_RIGHT: Self = ic("\u{e5c8}");
    pub const BRING_BACK: Self = ic("\u{e5cb}");
    pub const BRING_TO_BACK: Self = ic("\u{e5dc}");
    pub const BRING_TO_FRONT: Self = ic("\u{e5dd}");
    pub const BRING_FRONT: Self = ic("\u{e5cc}");
    pub const BRUSH: Self = ic("\u{e3ae}");
    pub const BOLD: Self = ic("\u{e238}"); // Bold Text
    pub const CHECK: Self = ic("\u{e5ca}");
    pub const CHECK_CIRCLE: Self = ic("\u{e86c}"); // Check Circle
    pub const CANCEL: Self = ic("\u{e5c9}"); // Cancel
    pub const CANCEL_PRESENTATION: Self = ic("\u{e0e9}"); // Cancel Presentation
    pub const CIRCLE: Self = ic("\u{ef4a}"); // Circle
    pub const CHEVRON_LEFT: Self = ic("\u{e5cb}"); // Chevron Left
    pub const CHEVRON_RIGHT: Self = ic("\u{e5cc}"); // Chevron Right
    pub const CLOSE: Self = ic("\u{e5cd}"); // Close
    pub const CODE: Self = ic("\u{e86f}"); // Code
    pub const CONTENT_COPY: Self = ic("\u{e14d}"); // Content Copy
    pub const CONTENT_CUT: Self = ic("\u{e14e}"); // Content Cut
    pub const CONTENT_PASTE: Self = ic("\u{e14f}"); // Content Paste
    pub const DOC_UNKNOWN: Self = ic("\u{e06f}"); // Note
    pub const DOC_TEXT: Self = ic("\u{e873}"); // Description
    pub const DONE: Self = ic("\u{e876}"); // Done
    pub const DRAW: Self = ic("\u{e746}"); // Draw
    pub const EDIT: Self = ic("\u{e254}"); // Mode Edit
    pub const EMPTY_INBOX: Self = ic("\u{f07e}"); // Upcoming
    pub const ERASER: Self = ic("\u{e6d0}"); // Upcoming
    pub const EXPLORE: Self = ic("\u{e569}"); // Mini map on canvas
    pub const DELETE: Self = ic("\u{e872}"); // Delete
    pub const FOLDER: Self = ic("\u{e2c7}"); // Folder
    pub const FOLDER_OPEN: Self = ic("\u{e2c8}"); // Folder Open
    pub const FULLSCREEN: Self = ic("\u{e5d0}");
    pub const FULLSCREEN_EXIT: Self = ic("\u{e5d1}");
    pub const GROUP: Self = ic("\u{e7ef}"); // Group
    pub const HISTORY: Self = ic("\u{e889}"); // History
    pub const HIGHLIGHTER: Self = ic("\u{e6d1}");
    pub const HIGHLIGHT_OFF: Self = ic("\u{e888}"); // Highlight Off
    pub const HEADER_1: Self = ic("\u{e262}"); // Header 11
    pub const TOGGLE_SIDEBAR: Self = ic("\u{f7e4}");
    pub const HAND: Self = ic("\u{f82f}"); // Selection tool
    pub const IMAGE: Self = ic("\u{e3f4}"); // Image
    pub const INFO: Self = ic("\u{e88e}");
    pub const ITALIC: Self = ic("\u{e23f}");
    pub const KEYBOARD_HIDE: Self = ic("\u{e31a}");
    pub const LINK: Self = ic("\u{e157}");
    pub const LOCK_OPEN: Self = ic("\u{e898}");
    pub const LOCK_CLOSED: Self = ic("\u{e897}");

    pub const MONEY: Self = ic("\u{e263}"); // Monetization On
    pub const MORE_VERT: Self = ic("\u{e5d4}");
    pub const NUMBER_LIST: Self = ic("\u{e242}"); // Number List
    pub const PLACE_ITEM: Self = ic("\u{f1f0}"); // Place Item
    pub const PEN: Self = ic("\u{e150}"); // Pencil
    pub const PREVIEW: Self = ic("\u{f1c5}"); // Preview
    pub const SETTINGS: Self = ic("\u{e8b8}"); // Settings
    pub const SPARKLE: Self = ic("\u{e65f}"); // Auto Awesome
    pub const SAVE: Self = ic("\u{e161}"); // Save
    pub const SCHEDULE: Self = ic("\u{e8b5}"); // Schedule
    pub const SEARCH: Self = ic("\u{e8b6}"); // Search
    pub const SYNC: Self = ic("\u{e863}"); // Auto-renew
    pub const SHARED_FOLDER: Self = ic("\u{e2c9}"); // Shared Folder
    pub const OFFLINE: Self = ic("\u{e2c1}"); // Sync Disabled
    pub const UPDATE_REQ: Self = ic("\u{e629}"); // Sync Problem
    pub const SYNC_PROBLEM: Self = ic("\u{e000}"); // Sync Problem
    pub const TODO_LIST: Self = ic("\u{e6b3}"); // Todo List
    pub const THUMBS_UP: Self = ic("\u{e8dc}"); // Thumbs Up
    pub const VERTICAL_SPLIT: Self = ic("\u{e949}"); // Vertical Split
    pub const VIDEO_LABEL: Self = ic("\u{e071}"); // Video Label
    pub const VISIBILITY_ON: Self = ic("\u{e8f4}"); // Visibility On
    pub const VISIBILITY_OFF: Self = ic("\u{e8f5}"); // Visibility Off
    pub const REDO: Self = ic("\u{e15A}");
    pub const UNDO: Self = ic("\u{e166}");
    pub const WARNING: Self = ic("\u{e001}");
    pub const ZOOM_IN: Self = ic("\u{e145}");
    pub const ZOOM_OUT: Self = ic("\u{e15b}");
    pub const STRIKETHROUGH: Self = ic("\u{e257}");
    pub const BULLET_LIST: Self = ic("\u{e241}");
    pub const INDENT: Self = ic("\u{e23e}");
    pub const DEINDENT: Self = ic("\u{e23d}");
    pub const BUG: Self = ic("\u{e868}");
    pub const LANGUAGE: Self = ic("\u{e894}");
    pub const LIGHT_BULB: Self = ic("\u{e0f0}");
    pub const WARNING_2: Self = ic("\u{e002}");
    pub const FEEDBACK: Self = ic("\u{e87f}");
    pub const REPORT: Self = ic("\u{e160}");

    //pub const ARTICLE: Self = ic("\u{ef42}");
    //pub const COMMAND_KEY: Self = Self('\u{eae7}');
    //pub const SWAP_HORIZONTAL: Self = Self('\u{e933}'); // Swap Horizontal Circle
    //pub const EDIT_OFF: Self = ic("\u{e950}"); // Edit Off
    //pub const FIND_REPLACE: Self = ic("\u{e881}"); // Find Replace
    //pub const SHIELD: Self = Self("\u{e8e8}");
    //pub const SHIELD_OFF: Self = Self("\u{e9d4}");
    pub const RECTANGLE: Self = ic("\u{eb54}"); // Rectangle
    //pub const PALETTE: Self = ic("\u{e40a}"); // Palette
    //pub const QR_CODE: Self = ic("\u{e00a}"); // Qr Code 2

    pub fn color(self, color: egui::Color32) -> Self {
        let mut this = self;
        this.color = Some(color);
        this
    }

    pub fn size(self, sz: f32) -> Self {
        let mut this = self;
        this.size = sz;
        this
    }
    pub fn badge(self, has_badge: bool) -> Self {
        let mut this = self;
        this.has_badge = has_badge;
        this
    }
    pub fn weak(self, weak: bool) -> Self {
        Self { weak, ..self }
    }
}

impl From<&Icon> for egui::WidgetText {
    fn from(ic: &Icon) -> egui::WidgetText {
        let mut rt = egui::RichText::new(ic.icon).font(egui::FontId::monospace(ic.size));
        if let Some(color) = ic.color {
            rt = rt.color(color);
        }
        if ic.weak {
            rt = rt.weak();
        }

        rt.into()
    }
}

impl Icon {
    pub fn show(&self, ui: &mut egui::Ui) -> egui::Response {
        self.inner_show(ui, None)
    }
    pub fn paint(&self, ui: &mut egui::Ui, painter: &egui::Painter) -> egui::Response {
        self.inner_show(ui, Some(painter))
    }

    fn inner_show(&self, ui: &mut egui::Ui, painter: Option<&egui::Painter>) -> egui::Response {
        let padding = egui::vec2(0.0, 0.0);
        let desired_size = egui::vec2(self.size + padding.x, self.size + padding.y);

        let (rect, resp) = ui.allocate_at_least(desired_size, egui::Sense::hover());

        if ui.is_rect_visible(rect) {
            let text_color = ui.style().interact(&resp).text_color();
            let wrap_width = ui.available_width();

            let icon_pos = egui::pos2(rect.min.x + padding.x, rect.center().y - self.size / 2.0);

            let icon: egui::WidgetText = self.into();
            let icon =
                icon.into_galley(ui, Some(TextWrapMode::Extend), wrap_width, egui::TextStyle::Body);

            painter
                .unwrap_or(ui.painter())
                .galley(icon_pos, icon, text_color);
        }

        resp
    }
}
