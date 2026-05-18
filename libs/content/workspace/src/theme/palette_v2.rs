use egui::{
    Color32, Id, Visuals,
    style::{self},
};
use epaint::hex_color;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct Theme {
    #[serde(skip)]
    pub current: Mode,

    pub dim: ThemeVariant,
    pub light_prefs: Preferences,

    pub bright: ThemeVariant,
    pub dark_prefs: Preferences,
}

#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct ThemeVariant {
    #[serde(with = "color32_hex")]
    pub black: Color32,
    #[serde(with = "color32_hex")]
    pub grey: Color32,
    #[serde(with = "color32_hex")]
    pub red: Color32,
    #[serde(with = "color32_hex")]
    pub green: Color32,
    #[serde(with = "color32_hex")]
    pub yellow: Color32,
    #[serde(with = "color32_hex")]
    pub blue: Color32,
    #[serde(with = "color32_hex")]
    pub magenta: Color32,
    #[serde(with = "color32_hex")]
    pub cyan: Color32,
    #[serde(with = "color32_hex")]
    pub white: Color32,
}

#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct Preferences {
    pub primary: Palette,
    pub secondary: Palette,
    pub tertiary: Palette,
    pub quaternary: Palette,
}

mod color32_hex {
    use egui::Color32;
    use serde::{self, Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(color: &Color32, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let hex = format!("#{:02X}{:02X}{:02X}", color.r(), color.g(), color.b());
        serializer.serialize_str(&hex)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Color32, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let s = s.trim_start_matches('#');
        if s.len() != 6 {
            return Err(serde::de::Error::custom("expected 6 hex digits"));
        }
        let r = u8::from_str_radix(&s[0..2], 16).map_err(serde::de::Error::custom)?;
        let g = u8::from_str_radix(&s[2..4], 16).map_err(serde::de::Error::custom)?;
        let b = u8::from_str_radix(&s[4..6], 16).map_err(serde::de::Error::custom)?;
        Ok(Color32::from_rgb(r, g, b))
    }
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
            Mode::Light => self.dim,
            Mode::Dark => self.bright,
        }
    }

    pub fn bg(&self) -> ThemeVariant {
        match self.current {
            Mode::Light => self.bright,
            Mode::Dark => self.dim,
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
            Mode::Light => self.bright.black,
            Mode::Dark => self.dim.white,
        }
    }

    /// Returns the secondary foreground neutral color i.e. off-black in light
    /// mode, off-white in dark mode. Used for de-emphasized foreground elements
    /// like markdown list markers and file tree icons.
    pub fn neutral_fg_secondary(&self) -> Color32 {
        match self.current {
            Mode::Light => self.bright.grey.lerp_to_gamma(self.bright.black, 0.5),
            Mode::Dark => self.dim.grey.lerp_to_gamma(self.dim.white, 0.5),

        }
    }

    /// Returns the true neutral color i.e. grey in either mode. Used for
    /// greyed-out text, borders and strokes, and hovered widget backgrounds.
    pub fn neutral(&self) -> Color32 {
        match self.current {
            Mode::Light => self.bright.grey.lerp_to_gamma(self.bright.black, 0.3),
            Mode::Dark => self.dim.grey.lerp_to_gamma(self.dim.white, 0.25),
        }
    }

    /// Returns the tertiary foreground neutral color i.e. off-white in light
    /// mode, off-black in dark mode. Used for buttons and other widgets that
    /// need a background distinct from the UI background.
    pub fn neutral_bg_tertiary(&self) -> Color32 {
        match self.current {
            Mode::Light => self.bright.grey.lerp_to_gamma(self.bright.black, 0.15),
            Mode::Dark => self.dim.grey.lerp_to_gamma(self.dim.white, 0.15),
        }
    }

    /// Returns the secondary foreground neutral color i.e. off-white in light
    /// mode, off-black in dark mode. Used for UI background in most places.
    pub fn neutral_bg_secondary(&self) -> Color32 {
        match self.current {
            Mode::Light => self.bright.grey,
            Mode::Dark => self.dim.grey,
        }
    }

    /// Returns the background neutral color i.e. white in light mode,
    /// near-black in dark mode. Used for areas with editable content, similar
    /// to egui's `extreme_bg_color`.
    pub fn neutral_bg(&self) -> Color32 {
        match self.current {
            Mode::Light => self.bright.white,
            Mode::Dark => self.dim.black,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Mode {
    #[default]
    Light,
    Dark,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
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
        Self::default_theme(current)
    }

    pub fn with_mode(mut self, mode: Mode) -> Self {
        self.current = mode;
        self
    }

    /// The default theme is mnemonic by travis
    pub fn default_theme(current: Mode) -> Self {
        Self {
            current,
            dim: ThemeVariant {
                black: hex_color!("#101010"),
                red: hex_color!("#DF2040"),
                green: hex_color!("#00B371"),
                yellow: hex_color!("#E6AC00"),
                blue: hex_color!("#207FDF"),
                magenta: hex_color!("#7855AA"),
                cyan: hex_color!("#00BBCC"),
                white: hex_color!("#FFFFFF"),
                grey: hex_color!("#1D1D1D"),
            },
            light_prefs: Preferences {
                primary: Palette::Blue,
                secondary: Palette::Green,
                tertiary: Palette::Magenta,
                quaternary: Palette::Cyan,
            },
            bright: ThemeVariant {
                black: hex_color!("#101010"),
                grey: hex_color!("#F6F6F6"),
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
            dim: ThemeVariant {
                black: hex_color!("#1C1B22"),
                grey: hex_color!("#353745"),
                white: hex_color!("#FFFFFF"),
                red: hex_color!("#CB3A2A"),
                green: hex_color!("#14710A"),
                yellow: hex_color!("#FFB86C"),
                blue: hex_color!("#036A96"),
                magenta: hex_color!("#A3144D"),
                cyan: hex_color!("#036A96"),
            },
            dark_prefs: Preferences {
                primary: Palette::Magenta,
                secondary: Palette::Green,
                tertiary: Palette::Yellow,
                quaternary: Palette::Cyan,
            },
            bright: ThemeVariant {
                black: hex_color!("#1F1F1F"),
                white: hex_color!("#FFFBEB"),
                grey: hex_color!("#CFCFDE"),
                red: hex_color!("#FF5555"),
                green: hex_color!("#50FA7B"),
                yellow: hex_color!("#F1FA8C"),
                blue: hex_color!("#8BE9FD"),
                magenta: hex_color!("#FF79C6"),
                cyan: hex_color!("#8BE9FD"),
            },
            light_prefs: Preferences {
                primary: Palette::Magenta,
                secondary: Palette::Green,
                tertiary: Palette::Yellow,
                quaternary: Palette::Cyan,
            },
        }
    }

    pub fn catppuccin(current: Mode) -> Self {
        Self {
            current,
            bright: ThemeVariant {
                black: hex_color!("#11111b"),
                red: hex_color!("#f38ba8"),
                green: hex_color!("#a6e3a1"),
                yellow: hex_color!("#f9e2af"),
                blue: hex_color!("#89b4fa"),
                magenta: hex_color!("#eba0ac"),
                cyan: hex_color!("#94e2d5"),
                white: hex_color!("#cdd6f4"),
                grey: hex_color!("#7f849c"),
            },
            dark_prefs: Preferences {
                primary: Palette::Red,
                secondary: Palette::Magenta,
                tertiary: Palette::Green,
                quaternary: Palette::Blue,
            },
            dim: ThemeVariant {
                black: hex_color!("#4c4f69"),
                grey: hex_color!("#8c8fa1"),
                red: hex_color!("#d20f39"),
                green: hex_color!("#40a02b"),
                yellow: hex_color!("#df8e1d"),
                blue: hex_color!("#1e66f5"),
                magenta: hex_color!("#e64553"),
                cyan: hex_color!("#179299"),
                white: hex_color!("#eff1f5"),
            },
            light_prefs: Preferences {
                primary: Palette::Red,
                secondary: Palette::Magenta,
                tertiary: Palette::Green,
                quaternary: Palette::Blue,
            },
        }
    }
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

/// Deterministic palette for a username — FNV-like rolling hash picks one of
/// six hues so a user has a stable color across sessions without persistence.
pub fn username_color(name: &str) -> Palette {
    const COLORS: [Palette; 6] = [
        Palette::Red,
        Palette::Green,
        Palette::Yellow,
        Palette::Blue,
        Palette::Magenta,
        Palette::Cyan,
    ];
    let hash = name
        .bytes()
        .fold(0u32, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u32));
    COLORS[hash as usize % COLORS.len()]
}
