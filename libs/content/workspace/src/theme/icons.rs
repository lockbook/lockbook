use egui::TextWrapMode;

#[derive(Copy, Clone, PartialEq)]
pub struct Icon {
    pub has_badge: bool,
    pub icon: &'static str,
    pub size: f32,
    color: Option<egui::Color32>,
    weak: bool,
    frame: bool,
}

const fn ic(c: &'static str) -> Icon {
    Icon { has_badge: false, icon: c, size: 18.0, color: None, weak: false, frame: false }
}

// look em up here: https://www.nerdfonts.com/cheat-sheet
// if you have nerdfonts installed the previews in the comments should be accurate
// make duplicates clear in the code
// don't leave dead code behind
impl Icon {
    pub const ARROW_LEFT: Self = ic("\u{f060}"); // 
    pub const ARROW_RIGHT: Self = ic("\u{f061}"); // 
    pub const BRING_TO_BACK: Self = ic("\u{f0600}"); // 󰘀
    pub const BRING_TO_FRONT: Self = ic("\u{f0601}"); // 󰘁
    pub const BRUSH: Self = ic("\u{f1a0d}"); // 󰃣
    pub const CHAT: Self = ic("\u{f27a}"); // nf-fa-message
    pub const CHECK_CIRCLE: Self = ic("\u{f05e0}"); // 󰗠
    pub const CIRCLE: Self = ic("\u{eabc}"); // 
    pub const CHEVRON_LEFT: Self = ic("\u{f0141}"); // 󰅁
    pub const CHEVRON_RIGHT: Self = ic("\u{f0142}"); // 󰅂
    pub const CHEVRON_UP: Self = ic("\u{f0143}"); // 󰅃
    pub const CHEVRON_DOWN: Self = ic("\u{f0140}"); // 󰅀
    pub const CLOSE: Self = ic("\u{f0156}"); // 󰅖
    pub const CODE: Self = ic("\u{f0174}"); // 󰅴
    pub const CONTENT_COPY: Self = ic("\u{f018f}"); // 󰆏
    pub const CONTENT_CUT: Self = ic("\u{f0190}"); // 󰆐
    pub const DOC_UNKNOWN: Self = ic("\u{f039a}"); // 󰎚
    pub const DOC_TEXT: Self = ic("\u{f15c}"); // 
    pub const DOC_MD: Self = ic("\u{f48a}"); // 
    pub const DOC_PDF: Self = ic("\u{e67d}"); // 
    pub const DONE: Self = ic("\u{f012c}"); // 󰄬
    pub const DOTS_HORIZONTAL: Self = ic("\u{f01d8}"); // 󰇘
    pub const DRAW: Self = Self::BRUSH;
    pub const ERASER: Self = ic("\u{f01fe}"); // 󰙂
    pub const FOLDER: Self = ic("\u{f024b}"); // 󰉋
    pub const FULLSCREEN: Self = ic("\u{f0293}"); // 󰊓
    pub const FULLSCREEN_EXIT: Self = ic("\u{f0294}"); // 󰊔
    pub const PENCIL: Self = ic("\u{f0cb6}"); // 󰲶
    pub const TOGGLE_SIDEBAR: Self = ic("\u{ebf3}"); // 
    pub const HAND: Self = ic("\u{f01bf}"); // 
    pub const IMAGE: Self = ic("\u{f02e9}"); // 󰋩
    pub const NO_IMAGE: Self = ic("\u{F11D1}"); // 󱇑
    pub const OPEN_IN_NEW: Self = ic("\u{f03cc}"); // 󰏌
    pub const LOCK_OPEN: Self = ic("\u{f033f}"); // 󰌿
    pub const LOCK_CLOSED: Self = ic("\u{f033e}"); // 󰌾
    pub const SAVE: Self = ic("\u{f0193}"); // 󰆓
    pub const SCHEDULE: Self = ic("\u{f0954}"); // 󰥔
    pub const SEND: Self = ic("\u{f1d8}"); // nf-fa-send
    pub const SEARCH: Self = ic("\u{e644}"); // 
    pub const FILTER: Self = ic("\u{f0232}"); // 󰈲
    pub const HOME: Self = ic("\u{f02dc}"); // 󰋜
    pub const SYNC: Self = ic("\u{f006a}"); // 󰁪
    pub const SHAPES: Self = ic("\u{f0832}"); // 󰠱
    pub const SYNC_PROBLEM: Self = ic("\u{f0026}"); // 󰀦
    pub const REDO: Self = ic("\u{f044f}"); // 󰑏
    pub const UNDO: Self = ic("\u{f054d}"); // 󰕍
    pub const ZOOM_IN: Self = ic("\u{f0415}"); // 󰐕
    pub const ZOOM_OUT: Self = ic("\u{f0374}"); // 󰍴
    pub const BUG: Self = ic("\u{f00e4}"); // 󰃤
    pub const LINE: Self = ic("\u{f45b}"); // 
    pub const RECTANGLE: Self = ic("\u{f0e5e}"); // 󰹞
    pub const FIT_WIDTH: Self = ic("\u{f0e74}"); // 󰹴
    pub const FIT_HEIGHT: Self = ic("\u{f0e79}"); // 󰹹

    pub fn color(self, color: egui::Color32) -> Self {
        let mut this = self;
        this.color = Some(color);
        this
    }

    pub fn frame(self, frame: bool) -> Self {
        let mut this = self;
        this.frame = frame;
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

        let (rect, resp) = ui.allocate_at_least(desired_size, egui::Sense::click_and_drag());

        if ui.is_rect_visible(rect) {
            let style = ui.style().interact(&resp);
            let text_color = style.text_color();
            let wrap_width = ui.available_width();

            let icon_pos = egui::pos2(rect.min.x + padding.x, rect.center().y - self.size / 2.0);

            let icon: egui::WidgetText = self.into();
            let icon =
                icon.into_galley(ui, Some(TextWrapMode::Extend), wrap_width, egui::TextStyle::Body);

            if self.frame {
                painter.unwrap_or(ui.painter()).rect_filled(
                    rect.expand2(ui.spacing().button_padding),
                    style.corner_radius,
                    style.bg_fill,
                );
            }

            painter
                .unwrap_or(ui.painter())
                .galley(icon_pos, icon, text_color);
        }

        resp
    }
}
