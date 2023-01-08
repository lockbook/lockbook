use egui::color::Hsva;
use egui::Color32;

// Apple colors: https://developer.apple.com/design/human-interface-guidelines/foundations/color/
pub const RED: ThemedColor =
    ThemedColor { light: Color32::from_rgb(255, 59, 48), dark: Color32::from_rgb(255, 69, 58) };
pub const ORANGE: ThemedColor =
    ThemedColor { light: Color32::from_rgb(255, 149, 0), dark: Color32::from_rgb(255, 159, 10) };
pub const YELLOW: ThemedColor =
    ThemedColor { light: Color32::from_rgb(255, 204, 0), dark: Color32::from_rgb(255, 214, 10) };
pub const GREEN: ThemedColor =
    ThemedColor { light: Color32::from_rgb(52, 199, 89), dark: Color32::from_rgb(48, 209, 88) };
pub const MINT: ThemedColor =
    ThemedColor { light: Color32::from_rgb(0, 199, 190), dark: Color32::from_rgb(102, 212, 207) };
pub const TEAL: ThemedColor =
    ThemedColor { light: Color32::from_rgb(48, 176, 199), dark: Color32::from_rgb(64, 200, 224) };
pub const CYAN: ThemedColor =
    ThemedColor { light: Color32::from_rgb(50, 173, 230), dark: Color32::from_rgb(100, 210, 255) };
pub const BLUE: ThemedColor =
    ThemedColor { light: Color32::from_rgb(0, 122, 255), dark: Color32::from_rgb(10, 132, 255) };
pub const INDIGO: ThemedColor =
    ThemedColor { light: Color32::from_rgb(88, 86, 214), dark: Color32::from_rgb(94, 92, 230) };
pub const PURPLE: ThemedColor =
    ThemedColor { light: Color32::from_rgb(175, 82, 222), dark: Color32::from_rgb(191, 90, 242) };
pub const PINK: ThemedColor =
    ThemedColor { light: Color32::from_rgb(255, 45, 85), dark: Color32::from_rgb(255, 55, 95) };
pub const BROWN: ThemedColor =
    ThemedColor { light: Color32::from_rgb(162, 132, 94), dark: Color32::from_rgb(172, 142, 104) };

// light theme semantics; `GRAY` is closest to `BLACK` and `GRAY_6` is closest to `WHITE`
pub const BLACK: ThemedColor = ThemedColor { light: Color32::BLACK, dark: Color32::WHITE };
pub const GRAY: ThemedColor =
    ThemedColor { light: Color32::from_rgb(142, 142, 147), dark: Color32::from_rgb(142, 142, 147) };
pub const GRAY_2: ThemedColor =
    ThemedColor { light: Color32::from_rgb(174, 174, 178), dark: Color32::from_rgb(99, 99, 102) };
pub const GRAY_3: ThemedColor =
    ThemedColor { light: Color32::from_rgb(199, 199, 204), dark: Color32::from_rgb(72, 72, 74) };
pub const GRAY_4: ThemedColor =
    ThemedColor { light: Color32::from_rgb(209, 209, 214), dark: Color32::from_rgb(58, 58, 60) };
pub const GRAY_5: ThemedColor =
    ThemedColor { light: Color32::from_rgb(229, 229, 234), dark: Color32::from_rgb(44, 44, 46) };
pub const GRAY_6: ThemedColor =
    ThemedColor { light: Color32::from_rgb(242, 242, 247), dark: Color32::from_rgb(28, 28, 30) };
pub const WHITE: ThemedColor = ThemedColor { light: Color32::WHITE, dark: Color32::BLACK };

/// provides a mechanism for the application developer to override colors for dark mode and light
/// mode and for us to provide defaults
#[derive(Default)]
pub struct VisualAppearance {
    pub current_theme: Theme,

    pub text: Option<ThemedColor>,
    pub selection_bg: Option<ThemedColor>,
    pub heading: Option<ThemedColor>,
    pub heading_line: Option<ThemedColor>,
    pub code: Option<ThemedColor>,
    pub bold: Option<ThemedColor>,
    pub italics: Option<ThemedColor>,
    pub strikethrough: Option<ThemedColor>,
    pub link: Option<ThemedColor>,
}

impl VisualAppearance {
    pub fn text(&self) -> Color32 {
        self.text.unwrap_or(BLACK).get(self.current_theme)
    }

    pub fn selection_bg(&self) -> Color32 {
        let mut color = BLUE;

        // light mode: use half saturation
        color.light = {
            let mut color_hsva = Hsva::from(color.light);
            color_hsva.s /= 2.0;
            Color32::from(color_hsva)
        };

        self.selection_bg.unwrap_or(color).get(self.current_theme)
    }

    pub fn heading(&self) -> Color32 {
        self.heading.unwrap_or(BLACK).get(self.current_theme)
    }

    pub fn heading_line(&self) -> Color32 {
        self.heading_line.unwrap_or(GRAY).get(self.current_theme)
    }

    pub fn code(&self) -> Color32 {
        self.code.unwrap_or(PINK).get(self.current_theme)
    }

    pub fn bold(&self) -> Color32 {
        self.bold.unwrap_or(BLACK).get(self.current_theme)
    }

    pub fn italics(&self) -> Color32 {
        self.italics.unwrap_or(BLACK).get(self.current_theme)
    }

    pub fn strikethrough(&self) -> Color32 {
        self.strikethrough.unwrap_or(PINK).get(self.current_theme)
    }

    pub fn link(&self) -> Color32 {
        self.link.unwrap_or(BLUE).get(self.current_theme)
    }

    pub fn update(&mut self, ui: &egui::Ui) {
        self.current_theme = if ui.visuals().dark_mode {
            Theme::Dark
        } else {
            Theme::Light
        }
    }
}

#[derive(Default, Clone, Copy)]
pub enum Theme {
    #[default]
    Dark,

    Light,
}

#[derive(Clone, Copy)]
pub struct ThemedColor {
    pub light: Color32,
    pub dark: Color32,
}

impl ThemedColor {
    pub fn get(&self, theme: Theme) -> Color32 {
        match theme {
            Theme::Dark => self.dark,
            Theme::Light => self.light,
        }
    }
}
