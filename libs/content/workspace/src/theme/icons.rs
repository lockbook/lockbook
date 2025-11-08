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

// look em up here: https://www.nerdfonts.com/cheat-sheet
// if you have nerdfonts installed the previews in the comments should be accurate
// make duplicates clear in the code
// don't leave dead code behind
impl Icon {
    pub const ACCOUNT: Self = ic("\u{f0004}"); // 󰀄
    pub const ARROW_DOWN: Self = ic("\u{f035d}"); // 󰍝
    pub const ARROW_UP: Self = ic("\u{f0360}"); // 󰍠
    pub const ARROW_LEFT: Self = ic("\u{e5de}"); // 󰍜
    pub const ARROW_RIGHT: Self = ic("\u{e5df}"); // 󰍜
    pub const BRING_TO_BACK: Self = ic("\u{f0600}");
    pub const BRING_TO_FRONT: Self = ic("\u{f0601}");
    pub const BRING_FRONT: Self = ic("\u{e5cc}");
    pub const BRING_BACK: Self = ic("\u{e5cb}");
    pub const BRUSH: Self = ic("\u{f00e3}");
    pub const BOLD: Self = ic("\u{f0264}");
    pub const CHECK_CIRCLE: Self = ic("\u{f05e0}");
    pub const CIRCLE: Self = ic("\u{eabc}");
    pub const CHEVRON_LEFT: Self = ic("\u{f0141}");
    pub const CHEVRON_RIGHT: Self = ic("\u{f0142}");
    pub const CLOSE: Self = ic("\u{f0156}");
    pub const CODE: Self = ic("\u{f0174}");
    pub const CONTENT_COPY: Self = ic("\u{f018f}");
    pub const CONTENT_CUT: Self = ic("\u{f0190}");
    pub const CONTENT_PASTE: Self = ic("\u{f0192}");
    pub const DOC_UNKNOWN: Self = ic("\u{f039a}");
    pub const DOC_TEXT: Self = ic("\u{f15c}"); // 
    pub const DOC_MD: Self = ic("\u{f48a}");
    pub const DONE: Self = ic("\u{f012c}");
    pub const DRAW: Self = Self::BRUSH;
    pub const EMPTY_INBOX: Self = ic("\u{f06ee}");
    pub const ERASER: Self = ic("\u{f0642}");
    pub const DELETE: Self = ic("\u{f01b4}");
    pub const FOLDER: Self = ic("\u{f024b}");
    pub const FOLDER_OPEN: Self = ic("\u{f0770}");
    pub const FULLSCREEN: Self = ic("\u{f0293}");
    pub const FULLSCREEN_EXIT: Self = ic("\u{f0294}");
    pub const GROUP: Self = ic("\u{f0c0}");
    pub const HIGHLIGHTER: Self = ic("\u{f0652}");
    pub const HEADER_1: Self = ic("\u{f026b}");
    pub const TOGGLE_SIDEBAR: Self = ic("\u{ebf3}");
    pub const HAND: Self = ic("\u{f245}");
    pub const IMAGE: Self = ic("\u{f02e9}");
    pub const INFO: Self = ic("\u{f02fc}");
    pub const ITALIC: Self = ic("\u{f0277}");
    pub const KEYBOARD_HIDE: Self = ic("\u{f030f}");
    pub const LINK: Self = ic("\u{f0337}");
    pub const OPEN_IN_NEW: Self = ic("\u{f03cc}");
    pub const LOCK_OPEN: Self = ic("\u{f033f}");
    pub const LOCK_CLOSED: Self = ic("\u{f033e}");
    pub const MONEY: Self = ic("\u{ef8d}");
    pub const NUMBER_LIST: Self = ic("\u{f027b}");
    pub const SETTINGS: Self = ic("\u{f0493}");
    pub const SPARKLE: Self = ic("\u{f51b}");
    pub const SAVE: Self = ic("\u{f0193}");
    pub const SCHEDULE: Self = ic("\u{f0954}");
    pub const SEARCH: Self = ic("\u{e644}");
    pub const SYNC: Self = ic("\u{f006a}");
    pub const SHARED_FOLDER: Self = ic("\u{f024c}");
    pub const SHAPES: Self = ic("\u{f0831}");
    pub const OFFLINE: Self = ic("\u{f4ad}");
    pub const UPDATE_REQ: Self = ic("\u{f04e7}");
    pub const SYNC_PROBLEM: Self = ic("\u{f0026}");
    pub const TODO_LIST: Self = ic("\u{f096a}");
    pub const THUMBS_UP: Self = ic("\u{f0513}");
    pub const REDO: Self = ic("\u{f044f}");
    pub const UNDO: Self = ic("\u{f054d}");
    pub const ZOOM_IN: Self = ic("\u{f0415}"); // 󰐕
    pub const ZOOM_OUT: Self = ic("\u{f0374}"); // 󰍴
    pub const STRIKETHROUGH: Self = ic("\u{f0280}");
    pub const BULLET_LIST: Self = ic("\u{f0279}");
    pub const INDENT: Self = ic("\u{f0276}");
    pub const DEINDENT: Self = ic("\u{f0275}");
    pub const BUG: Self = ic("\u{f00e4}");
    pub const LANGUAGE: Self = ic("\u{f0ac}");
    pub const LIGHT_BULB: Self = ic("\u{f400}");
    pub const LINE: Self = ic("\u{f45b}");
    pub const WARNING_2: Self = Self::SYNC_PROBLEM;
    pub const FEEDBACK: Self = ic("\u{f0b7a}");
    pub const REPORT: Self = ic("\u{f46e}");
    pub const RECTANGLE: Self = ic("\u{f0e5e}");

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
