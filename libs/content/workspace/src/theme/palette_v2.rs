use egui::{
    Color32, Id,
    style::{self},
};
use epaint::hex_color;

#[derive(Clone, Copy)]
pub struct Theme {
    pub current: Mode,

    light: ThemeVariant,
    light_prefs: Preferences,

    dark: ThemeVariant,
    dark_prefs: Preferences,
}

#[derive(Clone, Copy)]
pub struct ThemeVariant {
    pub black: Color32,
    pub grey: Color32,
    pub red: Color32,
    pub green: Color32,
    pub yellow: Color32,
    pub blue: Color32,
    pub magenta: Color32,
    pub cyan: Color32,
    pub white: Color32,
}

#[derive(Clone, Copy)]
pub struct Preferences {
    pub primary: Palette,
    pub secondary: Palette,
    pub tertiary: Palette,
    pub quaternary: Palette,
}

impl Theme {
    pub fn prefs(&self) -> Preferences {
        match self.current {
            Mode::Light => self.light_prefs,
            Mode::Dark => self.dark_prefs,
        }
    }

    pub fn fg(&self) -> ThemeVariant {
        match self.current {
            Mode::Light => self.light,
            Mode::Dark => self.dark,
        }
    }

    pub fn bg(&self) -> ThemeVariant {
        match self.current {
            Mode::Light => self.dark,
            Mode::Dark => self.light,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Light,
    Dark,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
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

impl ThemeVariant {
    pub fn get_color(&self, p: Palette) -> Color32 {
        match p {
            Palette::Foreground => unreachable!(),
            Palette::Background => unreachable!(),
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

impl Theme {
    pub fn default(current: Mode) -> Self {
        Self::travis_prophecy(current)
    }

    pub fn travis_prophecy(current: Mode) -> Self {
        Self {
            current,
            light: ThemeVariant {
                black: hex_color!("#101010"),
                red: hex_color!("#DF2040"),
                green: hex_color!("#2DD296"),
                yellow: hex_color!("#FFBF00"),
                blue: hex_color!("#207FDF"),
                magenta: hex_color!("#7855AA"),
                cyan: hex_color!("#13DAEC"),
                white: hex_color!("#FFFFFF"),
                grey: hex_color!("#1A1A1A"),
            },
            light_prefs: Preferences {
                primary: Palette::Blue,
                secondary: Palette::Magenta,
                tertiary: Palette::Cyan,
                quaternary: Palette::Green,
            },
            dark: ThemeVariant {
                black: hex_color!("#101010"),
                grey: hex_color!("#F0F0F0"),
                red: hex_color!("#FF6680"),
                green: hex_color!("#67E4B6"),
                yellow: hex_color!("#FFDB70"),
                blue: hex_color!("#66B2FF"),
                magenta: hex_color!("#AC8CD9"),
                cyan: hex_color!("#6EECF7"),
                white: hex_color!("#FFFFFF"),
            },
            dark_prefs: Preferences {
                primary: Palette::Blue,
                secondary: Palette::Magenta,
                tertiary: Palette::Cyan,
                quaternary: Palette::Green,
            },
        }
    }

    pub fn darcula(current: Mode) -> Self {
        Self {
            current,
            dark: ThemeVariant {
                black: hex_color!("#5E5E5E"),
                red: hex_color!("#972F4D"),
                green: hex_color!("#628D54"),
                yellow: hex_color!("#ACA47D"),
                blue: hex_color!("#5F4BC1"),
                magenta: hex_color!("#9F395B"),
                cyan: hex_color!("#4277A0"),
                white: hex_color!("#F5F5F5"),
                grey: hex_color!("#E6E6E6"),
            },
            light_prefs: Preferences {
                primary: Palette::Blue,
                secondary: Palette::Green,
                tertiary: Palette::Magenta,
                quaternary: Palette::Cyan,
            },
            light: ThemeVariant {
                black: hex_color!("#15131F"),
                grey: hex_color!("#23212B"),
                red: hex_color!("#D27DAC"),
                green: hex_color!("#A1EE8D"),
                yellow: hex_color!("#CBCD7B"),
                blue: hex_color!("#15131F"),
                magenta: hex_color!("#DABA82"),
                cyan: hex_color!("#8E7FE5"),
                white: hex_color!("#FAFAFA"),
            },
            dark_prefs: Preferences {
                primary: Palette::Blue,
                secondary: Palette::Green,
                tertiary: Palette::Magenta,
                quaternary: Palette::Cyan,
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
        let (fg, bg) = if self.current == Mode::Light {
            (self.fg().black, self.bg().white)
        } else {
            (self.bg().white, self.bg().black)
        };

        let mut base = egui::Visuals {
            dark_mode: self.current == Mode::Dark,
            override_text_color: None,
            window_fill: self.bg().grey,
            extreme_bg_color: bg, // will need light mode
            // switch
            selection: style::Selection {
                bg_fill: self.bg().get_color(self.prefs().primary),
                ..Default::default()
            },
            hyperlink_color: self.fg().get_color(self.prefs().secondary),
            faint_bg_color: bg.gamma_multiply(0.9),
            code_bg_color: self.bg().grey,
            warn_fg_color: self.fg().yellow,
            error_fg_color: self.fg().red,
            panel_fill: self.bg().grey,
            ..Default::default()
        };

        base.widgets.noninteractive.bg_fill = self.bg().grey;
        base.widgets.noninteractive.weak_bg_fill = self.bg().grey;
        base.widgets.noninteractive.fg_stroke.color = fg;
        base.widgets.noninteractive.bg_stroke.color = self.bg().grey;

        base.widgets.inactive.bg_fill = self.bg().grey;
        base.widgets.inactive.weak_bg_fill = self.bg().grey;
        base.widgets.inactive.fg_stroke.color = fg;
        base.widgets.inactive.bg_stroke.color = self.bg().grey;

        base.widgets.hovered.bg_fill = self.bg().grey.lerp_to_gamma(bg, 0.5);
        base.widgets.hovered.weak_bg_fill = self.bg().grey.lerp_to_gamma(bg, 0.5);
        base.widgets.hovered.fg_stroke.color = fg;
        base.widgets.hovered.bg_stroke.color = self.bg().grey.lerp_to_gamma(bg, 0.5);

        base.widgets.active.bg_fill = self.bg().get_color(self.prefs().primary);
        base.widgets.active.weak_bg_fill = self.bg().grey;
        base.widgets.active.fg_stroke.color = fg;
        base.widgets.active.bg_stroke.color = self.bg().grey;

        base.widgets.open.bg_fill = self.bg().grey;
        base.widgets.open.weak_bg_fill = self.bg().grey;
        base.widgets.open.fg_stroke.color = fg;
        base.widgets.open.bg_stroke.color = self.bg().grey;

        base
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
