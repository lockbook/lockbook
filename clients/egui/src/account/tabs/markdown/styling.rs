use eframe::egui;
use pulldown_cmark::HeadingLevel;

pub struct Styling {
    font_size: f32,
    code: bool,
    italics: bool,
    weak: bool,
}

impl Default for Styling {
    fn default() -> Self {
        Self { font_size: 17.0, italics: false, code: false, weak: false }
    }
}

impl Styling {
    pub fn gen_rich_text(&self, content: &str) -> egui::RichText {
        let mut txt = egui::RichText::new(content).font(egui::FontId::proportional(self.font_size));
        if self.code {
            txt = txt.code();
        }
        if self.italics {
            txt = txt.italics();
        }
        if self.weak {
            txt = txt.weak();
        }
        txt
    }

    pub fn set_for_heading(&mut self, lvl: &HeadingLevel) {
        use HeadingLevel::*;

        self.font_size = match lvl {
            H1 => 40.0,
            H2 => 30.0,
            H3 => 20.0,
            H4 => 17.0,
            H5 => 15.0,
            H6 => 12.0,
        };
    }

    pub fn unset_heading(&mut self) {
        self.font_size = 17.0;
    }

    pub fn set_for_blockquote(&mut self) {
        self.italics = true;
        self.weak = true;
    }

    pub fn unset_blockquote(&mut self) {
        self.italics = false;
        self.weak = false;
    }

    pub fn set_for_code(&mut self) {
        self.code = true;
    }

    pub fn unset_code(&mut self) {
        self.code = false;
    }
}
