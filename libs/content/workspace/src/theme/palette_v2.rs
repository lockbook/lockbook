use egui::{Color32, Id, style};
use epaint::hex_color;

#[derive(Clone, Copy)]
pub struct Theme {
    current: Mode,
    light: ThemeVariant,
    dark: ThemeVariant,
}

#[derive(Clone, Copy)]
pub struct ThemeVariant {
    pub black: Color32,
    pub red: Color32,
    pub green: Color32,
    pub yellow: Color32,
    pub blue: Color32,
    pub magenta: Color32,
    pub cyan: Color32,
    pub white: Color32,

    pub bg: Palette,
    pub fg: Palette,
    pub primary: Palette,
    pub secondary: Palette,
    pub tertiary: Palette,
    pub quaternary: Palette,
}

impl Theme {
    pub fn fg(&self) -> &ThemeVariant {
        match self.current {
            Mode::Light => &self.light,
            Mode::Dark => &self.dark,
        }
    }

    pub fn bg(&self) -> &ThemeVariant {
        match self.current {
            Mode::Light => &self.light,
            Mode::Dark => &self.dark,
        }
    }
}

impl ThemeVariant {
    pub fn from_palette(&self, p: Palette) -> Color32 {
        match p {
            Palette::Foreground => self.from_palette(self.fg),
            Palette::Background => self.from_palette(self.bg),
            Palette::Black => self.black,
            Palette::Red => self.red,
            Palette::Green => self.green,
            Palette::Yellow => self.yellow,
            Palette::Blue => self.blue,
            Palette::Magenta => self.magenta,
            Palette::Cyan => self.cyan,
            Palette::White => self.white,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Light,
    Dark,
}

#[derive(Clone, Copy)]
pub enum Palette {
    Foreground,
    Background,
    Black,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    White,
}

impl Theme {
    pub fn mnemonic(current: Mode) -> Self {
        Self {
            current,
            light: ThemeVariant {
                black: hex_color!("#505050"),
                red: hex_color!("#DF2040"),
                green: hex_color!("#2DD296"),
                yellow: hex_color!("#FFBF00"),
                blue: hex_color!("#207FDF"),
                magenta: hex_color!("#7855AA"),
                cyan: hex_color!("#13DAEC"),
                white: hex_color!("#D0D0D0"),

                bg: Palette::White,
                fg: Palette::Blue,

                primary: Palette::Blue,
                secondary: Palette::Green,
                tertiary: Palette::Yellow,
                quaternary: Palette::Magenta,
            },
            dark: ThemeVariant {
                black: hex_color!("#808080"),
                red: hex_color!("#FF6680"),
                green: hex_color!("#67E4B6"),
                yellow: hex_color!("#FFDB70"),
                blue: hex_color!("#66B2FF"),
                magenta: hex_color!("#AC8CD9"),
                cyan: hex_color!("#6EECF7"),
                white: hex_color!("#F0F0F0"),

                bg: Palette::Black,
                fg: Palette::White,

                primary: Palette::Magenta,
                secondary: Palette::Blue,
                tertiary: Palette::Green,
                quaternary: Palette::Yellow,
            },
        }
    }
}

pub trait ThemeExt {
    fn set_theme(&self, t: Theme);
    fn get_theme(&self) -> Theme;
}

impl ThemeExt for egui::Context {
    fn set_theme(&self, t: Theme) {
        self.memory_mut(|m| m.data.insert_temp(Id::new("theme"), t));
        self.set_visuals(t.base_visuals());
    }

    fn get_theme(&self) -> Theme {
        self.memory_mut(|m| m.data.get_temp(Id::new("theme")))
            .unwrap()
    }
}

impl Theme {
    fn base_visuals(&self) -> egui::Visuals {
        egui::Visuals {
            dark_mode: self.current == Mode::Dark,
            override_text_color: None,
            window_fill: self.bg().black,
            extreme_bg_color: self.bg().black,
            ..Default::default()
            // widgets: style::Widgets {
            //     noninteractive: style::WidgetVisuals {
            //         bg_fill: (),
            //         weak_bg_fill: (),
            //         bg_stroke: (),
            //         rounding: (),
            //         fg_stroke: (),
            //         expansion: (),
            //     },
            //     inactive: (),
            //     hovered: (),
            //     active: (),
            //     open: (),
            // },
            // selection: (),
            // hyperlink_color: (),
            // faint_bg_color: (),
            // extreme_bg_color: (),
            // code_bg_color: (),
            // warn_fg_color: (),
            // error_fg_color: (),
            // window_rounding: (),
            // window_shadow: (),
            // window_fill: (),
            // window_stroke: (),
            // window_highlight_topmost: (),
            // menu_rounding: (),
            // panel_fill: (),
            // popup_shadow: (),
            // resize_corner_size: (),
            // text_cursor: (),
            // clip_rect_margin: (),
            // button_frame: (),
            // collapsing_header_frame: (),
            // indent_has_left_vline: (),
            // striped: (),
            // slider_trailing_fill: (),
            // handle_shape: (),
            // interact_cursor: (),
            // image_loading_spinners: (),
            // numeric_color_space: (),
        }
    }

    pub fn dark(&self) -> egui::Visuals {
        let mut v = egui::Visuals::dark();
        let is_mobile = cfg!(target_os = "ios") || cfg!(target_os = "android");

        if is_mobile {
            v.window_fill = Color32::from_rgb(0, 0, 0);
            v.extreme_bg_color = Color32::from_rgb(0, 0, 0);
        } else {
            v.window_fill = Color32::from_rgb(20, 20, 20);
            v.extreme_bg_color = Color32::from_rgb(20, 20, 20);
        }

        v.faint_bg_color = Color32::from_rgb(35, 35, 37);
        v.widgets.noninteractive.bg_fill = Color32::from_rgb(25, 25, 27);
        v.widgets.noninteractive.fg_stroke.color = Color32::from_rgb(242, 242, 247);
        v.widgets.inactive.fg_stroke.color = Color32::from_rgb(242, 242, 247);
        // v.widgets.active.bg_fill = ThemePalette::DARK[primary];

        v.widgets.hovered.bg_fill = v.code_bg_color.linear_multiply(0.1);

        v
    }

    pub fn light(&self) -> egui::Visuals {
        let mut v = egui::Visuals::light();
        v.window_fill = Color32::from_rgb(255, 255, 255);
        v.extreme_bg_color = Color32::from_rgb(255, 255, 255);
        // v.widgets.active.bg_fill = ThemePalette::LIGHT[primary];
        v.widgets.hovered.bg_fill = v.code_bg_color.linear_multiply(0.9);

        v
    }
}
