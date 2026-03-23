use egui::{
    Color32, Id, Visuals,
    style::{self},
};
use epaint::hex_color;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

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

    pub fn light(&self) -> bool {
        match self.current {
            Mode::Light => true,
            Mode::Dark => false,
        }
    }

    pub fn dark(&self) -> bool {
        !self.light()
    }

    /// Returns the foreground neutral color i.e. black in light mode, white in
    /// dark mode. Used for text and icons.
    pub fn neutral_fg(&self) -> Color32 {
        match self.current {
            Mode::Light => self.light.black,
            Mode::Dark => self.dark.white,
        }
    }

    /// Returns the secondary foreground neutral color i.e. off-black in light
    /// mode, off-white in dark mode. Used for de-emphasized foreground elements
    /// like markdown list markers and file tree icons.
    pub fn neutral_fg_secondary(&self) -> Color32 {
        match self.current {
            Mode::Light => self.light.black.lerp_to_gamma(self.light.grey, 0.5),
            Mode::Dark => self.dark.white.lerp_to_gamma(self.dark.grey, 0.5),
        }
    }

    /// Returns the true neutral color i.e. grey in either mode. Used for
    /// greyed-out text, borders and strokes, and hovered widget backgrounds.
    pub fn neutral(&self) -> Color32 {
        match self.current {
            Mode::Light => self.light.grey,
            Mode::Dark => self.dark.grey,
        }
    }

    /// Returns the tertiary foreground neutral color i.e. off-white in light
    /// mode, off-black in dark mode. Used for buttons and other widgets that
    /// need a background distinct from the UI background.
    pub fn neutral_bg_tertiary(&self) -> Color32 {
        match self.current {
            Mode::Light => self.light.white.lerp_to_gamma(self.light.grey, 0.8),
            Mode::Dark => self.dark.black.lerp_to_gamma(self.dark.grey, 0.8),
        }
    }

    /// Returns the secondary foreground neutral color i.e. off-white in light
    /// mode, off-black in dark mode. Used for UI background in most places.
    pub fn neutral_bg_secondary(&self) -> Color32 {
        match self.current {
            Mode::Light => self.light.white.lerp_to_gamma(self.light.grey, 0.2),
            Mode::Dark => self.dark.black.lerp_to_gamma(self.dark.grey, 0.2),
        }
    }

    /// Returns the background neutral color i.e. white in light mode,
    /// near-black in dark mode. Used for areas with editable content, similar
    /// to egui's `extreme_bg_color`.
    pub fn neutral_bg(&self) -> Color32 {
        match self.current {
            Mode::Light => self.light.white,
            Mode::Dark => self.dark.black,
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
        Self::travis(current)
    }

    pub fn travis(current: Mode) -> Self {
        Self {
            current,
            light: ThemeVariant {
                black: hex_color!("#000000"),
                red: hex_color!("#DF2040"),
                green: hex_color!("#00B371"),
                yellow: hex_color!("#E6AC00"),
                blue: hex_color!("#207FDF"),
                magenta: hex_color!("#7855AA"),
                cyan: hex_color!("#00BBCC"),
                white: hex_color!("#FFFFFF"),
                grey: hex_color!("#D0D0D0"),
            },
            light_prefs: Preferences {
                primary: Palette::Blue,
                secondary: Palette::Green,
                tertiary: Palette::Magenta,
                quaternary: Palette::Cyan,
            },
            dark: ThemeVariant {
                black: hex_color!("#101010"),
                grey: hex_color!("#505050"),
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
                secondary: Palette::Green,
                tertiary: Palette::Magenta,
                quaternary: Palette::Cyan,
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

const ACCENT_PALETTES: [Palette; 6] =
    [Palette::Red, Palette::Green, Palette::Yellow, Palette::Blue, Palette::Magenta, Palette::Cyan];

pub fn username_color(username: &str) -> Palette {
    let mut h = DefaultHasher::new();
    username.hash(&mut h);
    ACCENT_PALETTES[(h.finish() as usize) % ACCENT_PALETTES.len()]
}

pub trait ThemeExt {
    fn set_lb_theme(&self, t: Theme);
    fn get_lb_theme(&self) -> Theme;
}

impl ThemeExt for egui::Context {
    fn set_lb_theme(&self, t: Theme) {
        self.memory_mut(|m| m.data.insert_temp(Id::new("theme"), t));
        self.set_visuals(t.base_visuals());
    }

    fn get_lb_theme(&self) -> Theme {
        self.memory_mut(|m| m.data.get_temp(Id::new("theme")))
            .unwrap()
    }
}

impl Theme {
    fn base_visuals(&self) -> egui::Visuals {
        let mut base = egui::Visuals {
            dark_mode: self.current == Mode::Dark,
            override_text_color: None,
            window_fill: self.neutral_bg_secondary(),
            extreme_bg_color: self.neutral_bg(),
            selection: style::Selection {
                bg_fill: self.bg().get_color(self.prefs().primary),
                ..Default::default()
            },
            hyperlink_color: self.fg().get_color(self.prefs().secondary),
            faint_bg_color: self.neutral_bg_secondary(),
            code_bg_color: self.neutral_bg_secondary(),
            warn_fg_color: self.fg().yellow,
            error_fg_color: self.fg().red,
            panel_fill: self.neutral_bg_secondary(),
            ..if self.current == Mode::Light { Visuals::light() } else { Visuals::dark() }
        };

        base.widgets.noninteractive.bg_fill = self.neutral_bg_tertiary();
        base.widgets.noninteractive.weak_bg_fill = self.neutral_bg_tertiary();
        base.widgets.noninteractive.fg_stroke.color = self.neutral_fg();
        base.widgets.noninteractive.bg_stroke.color = self.neutral();

        base.widgets.inactive.bg_fill = self.neutral_bg_tertiary();
        base.widgets.inactive.weak_bg_fill = self.neutral_bg_tertiary();
        base.widgets.inactive.fg_stroke.color = self.neutral_fg();
        base.widgets.inactive.bg_stroke.color = self.neutral();

        base.widgets.hovered.bg_fill = self.neutral();
        base.widgets.hovered.weak_bg_fill = self.neutral();
        base.widgets.hovered.fg_stroke.color = self.neutral_fg();
        base.widgets.hovered.bg_stroke.color = self.neutral();

        base.widgets.active.bg_fill = self.bg().get_color(self.prefs().primary);
        base.widgets.active.weak_bg_fill = self.neutral_bg_tertiary();
        base.widgets.active.fg_stroke.color = self.neutral_fg();
        base.widgets.active.bg_stroke.color = self.neutral();

        base.widgets.open.bg_fill = self.neutral_bg_tertiary();
        base.widgets.open.weak_bg_fill = self.neutral_bg_tertiary();
        base.widgets.open.fg_stroke.color = self.neutral_fg();
        base.widgets.open.bg_stroke.color = self.neutral();

        base
    }
}
