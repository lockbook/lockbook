use egui::{
    Color32, Id, Stroke, Visuals,
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

    #[serde(default)]
    pub code: Option<CodeTheme>,
}

#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct CodeTheme {
    pub light: CodeVariant,
    pub dark: CodeVariant,
}

#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct CodeVariant {
    #[serde(with = "color32_hex")]
    pub background: Color32,
    #[serde(with = "color32_hex")]
    pub text: Color32,
    #[serde(with = "color32_hex")]
    pub comment: Color32,
    #[serde(with = "color32_hex")]
    pub class: Color32,
    #[serde(with = "color32_hex")]
    pub function: Color32,
    #[serde(with = "color32_hex")]
    pub keyword: Color32,
    #[serde(with = "color32_hex")]
    pub string: Color32,
    #[serde(with = "color32_hex")]
    pub number: Color32,
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

    /// Theme primary hue (selection, folders).
    pub fn accent(&self) -> Color32 {
        self.fg().get_color(self.prefs().primary)
    }

    /// Destructive ink — theme red.
    pub fn danger(&self) -> Color32 {
        self.fg().red
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

    pub fn code_variant(&self) -> Option<CodeVariant> {
        self.code.map(|code| match self.current {
            Mode::Light => code.light,
            Mode::Dark => code.dark,
        })
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

    /// Mid neutral (grey) — borders / strokes.
    pub fn neutral(&self) -> Color32 {
        match self.current {
            Mode::Light => self.bright.grey.lerp_to_gamma(self.bright.black, 0.3),
            Mode::Dark => self.dim.grey.lerp_to_gamma(self.dim.white, 0.25),
        }
    }

    /// Wash amount: rest → `neutral_fg` on hover.
    pub const HOVER_WASH: f32 = 0.05;
    /// Wash amount: rest → `neutral_fg` while pressed.
    pub const PRESS_WASH: f32 = 0.10;

    /// `rest` blended toward `neutral_fg` by `amount` (clamped 0..=1).
    pub fn wash_toward_neutral_fg(&self, rest: Color32, amount: f32) -> Color32 {
        rest.lerp_to_gamma(self.neutral_fg(), amount.clamp(0.0, 1.0))
    }

    /// Tertiary background — controls / raised plates on secondary bg.
    pub fn neutral_bg_tertiary(&self) -> Color32 {
        match self.current {
            Mode::Light => self.bright.grey.lerp_to_gamma(self.bright.black, 0.15),
            Mode::Dark => self.dim.grey.lerp_to_gamma(self.dim.white, 0.15),
        }
    }

    /// Secondary background — panels / chrome.
    pub fn neutral_bg_secondary(&self) -> Color32 {
        match self.current {
            Mode::Light => self.bright.grey,
            Mode::Dark => self.dim.grey,
        }
    }

    /// Primary background — editable content / extreme bg.
    pub fn neutral_bg(&self) -> Color32 {
        match self.current {
            Mode::Light => self.bright.white,
            Mode::Dark => self.dim.black,
        }
    }
}

/// Returns the color that, painted at opacity `alpha` over `background`,
/// composites to `target` (`target = alpha·x + (1−alpha)·background`,
/// solved for `x`). Painting the result over a *different* background lets
/// that background show through while keeping the `target` look over
/// `background` — e.g. a code pill that matches its opaque appearance over
/// the page yet lets a selection behind it bleed through. Solved in egui's
/// gamma (sRGB-byte) blend space; channels clamp to `[0, 255]`, so an
/// `alpha` too low to reach `target` degrades gracefully.
pub fn translucent_over(target: Color32, background: Color32, alpha: f32) -> Color32 {
    let a = alpha.clamp(f32::EPSILON, 1.0);
    let solve = |t: u8, b: u8| {
        (((t as f32) - (1.0 - a) * (b as f32)) / a)
            .round()
            .clamp(0.0, 255.0) as u8
    };
    Color32::from_rgba_unmultiplied(
        solve(target.r(), background.r()),
        solve(target.g(), background.g()),
        solve(target.b(), background.b()),
        (a * 255.0).round() as u8,
    )
}

pub fn ensure_normal_text_contrast(color: Color32, background: Color32) -> Color32 {
    if contrast_ratio(color, background).passes_normal_text() {
        return color;
    }

    let toward_black = contrast_ratio(Color32::BLACK, background);
    let toward_white = contrast_ratio(Color32::WHITE, background);
    let target = if toward_black >= toward_white { Color32::BLACK } else { Color32::WHITE };

    let mut lo = 0.0f32;
    let mut hi = 1.0f32;
    for _ in 0..12 {
        let mid = (lo + hi) / 2.0;
        if contrast_ratio(color.lerp_to_gamma(target, mid), background).passes_normal_text() {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    color.lerp_to_gamma(target, hi)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ContrastRatio {
    /// Below the 3:1 threshold used for large text and non-text UI.
    Low,
    /// Meets 3:1, used for large text and non-text UI.
    LargeText,
    /// Meets 4.5:1, used for normal text.
    NormalText,
    /// Maximum 21:1 contrast, pure black against pure white.
    Maximum,
}

impl ContrastRatio {
    pub fn passes_normal_text(self) -> bool {
        self >= Self::NormalText
    }
}

/// WCAG contrast threshold between two opaque colors.
pub fn contrast_ratio(a: Color32, b: Color32) -> ContrastRatio {
    let a = relative_luminance(a);
    let b = relative_luminance(b);
    let (lighter, darker) = if a >= b { (a, b) } else { (b, a) };
    let ratio = (lighter + 0.05) / (darker + 0.05);
    if (ratio - 21.0).abs() < 0.01 {
        ContrastRatio::Maximum
    } else if ratio >= 4.5 {
        ContrastRatio::NormalText
    } else if ratio >= 3.0 {
        ContrastRatio::LargeText
    } else {
        ContrastRatio::Low
    }
}

fn relative_luminance(color: Color32) -> f32 {
    fn channel(c: u8) -> f32 {
        let c = c as f32 / 255.0;
        if c <= 0.03928 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
    }

    0.2126 * channel(color.r()) + 0.7152 * channel(color.g()) + 0.0722 * channel(color.b())
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

impl TryFrom<&str> for Palette {
    type Error = ();

    fn try_from(name: &str) -> Result<Self, Self::Error> {
        match name {
            "black" => Ok(Palette::Black),
            "red" => Ok(Palette::Red),
            "green" => Ok(Palette::Green),
            "yellow" => Ok(Palette::Yellow),
            "blue" => Ok(Palette::Blue),
            "magenta" => Ok(Palette::Magenta),
            "cyan" => Ok(Palette::Cyan),
            "white" => Ok(Palette::White),
            _ => Err(()),
        }
    }
}

impl Theme {
    pub fn default(current: Mode) -> Self {
        Self::default_theme(current)
    }

    pub fn from_android_material(
        current: Mode, dim: ThemeVariant, light_prefs: Preferences, bright: ThemeVariant,
        dark_prefs: Preferences,
    ) -> Self {
        Self { current, dim, light_prefs, bright, dark_prefs, code: None }
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
            code: None,
        }
    }

    pub fn apple(current: Mode) -> Self {
        Self {
            current,
            dim: ThemeVariant {
                black: hex_color!("#000000"),
                grey: hex_color!("#1D1D1D"),
                white: hex_color!("#FFFFFF"),
                red: hex_color!("#FF383C"),
                green: hex_color!("#34C759"),
                yellow: hex_color!("#FFCC00"),
                blue: hex_color!("#006BC9"),
                magenta: hex_color!("#CB30E0"),
                cyan: hex_color!("#00C0E8"),
            },
            light_prefs: Preferences {
                primary: Palette::Blue,
                secondary: Palette::Green,
                tertiary: Palette::Magenta,
                quaternary: Palette::Cyan,
            },
            bright: ThemeVariant {
                black: hex_color!("#000000"),
                grey: hex_color!("#F6F6F6"),
                white: hex_color!("#FFFFFF"),
                red: hex_color!("#FF4245"),
                green: hex_color!("#30D158"),
                yellow: hex_color!("#FFD600"),
                blue: hex_color!("#0091FF"),
                magenta: hex_color!("#DB34F2"),
                cyan: hex_color!("#3CD3FE"),
            },
            dark_prefs: Preferences {
                primary: Palette::Blue,
                secondary: Palette::Green,
                tertiary: Palette::Magenta,
                quaternary: Palette::Cyan,
            },
            code: Some(CodeTheme {
                light: CodeVariant {
                    background: hex_color!("#FFFFFF"),
                    text: hex_color!("#000000"),
                    comment: hex_color!("#5D6C79"),
                    class: hex_color!("#1C464A"),
                    function: hex_color!("#326D74"),
                    keyword: hex_color!("#9B2393"),
                    string: hex_color!("#C41A16"),
                    number: hex_color!("#1C00CF"),
                },
                dark: CodeVariant {
                    background: hex_color!("#25252A"),
                    text: hex_color!("#FFFFFF"),
                    comment: hex_color!("#6C7986"),
                    class: hex_color!("#9EF1DD"),
                    function: hex_color!("#67B7A4"),
                    keyword: hex_color!("#FC5FA3"),
                    string: hex_color!("#FC6A5D"),
                    number: hex_color!("#D0BF69"),
                },
            }),
        }
    }

    pub fn apple_macos(current: Mode) -> Self {
        let mut theme = Self::apple(current);
        theme.dim.black = hex_color!("#101010");
        theme
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
                blue: hex_color!("#644AC9"),
                magenta: hex_color!("#A3144D"),
                cyan: hex_color!("#036A96"),
            },
            dark_prefs: Preferences {
                primary: Palette::Blue,
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
                blue: hex_color!("#BD93F9"),
                magenta: hex_color!("#FF79C6"),
                cyan: hex_color!("#8BE9FD"),
            },
            light_prefs: Preferences {
                primary: Palette::Blue,
                secondary: Palette::Green,
                tertiary: Palette::Yellow,
                quaternary: Palette::Cyan,
            },
            code: None,
        }
    }

    pub fn intellij(current: Mode) -> Self {
        Self {
            current,
            dim: ThemeVariant {
                black: hex_color!("#1E1F22"),
                grey: hex_color!("#2B2D30"),
                white: hex_color!("#DFE1E5"),
                red: hex_color!("#DB5860"),
                green: hex_color!("#59A869"),
                yellow: hex_color!("#C56823"),
                blue: hex_color!("#3574F0"),
                magenta: hex_color!("#871094"),
                cyan: hex_color!("#00627A"),
            },
            dark_prefs: Preferences {
                primary: Palette::Blue,
                secondary: Palette::Green,
                tertiary: Palette::Yellow,
                quaternary: Palette::Cyan,
            },
            bright: ThemeVariant {
                black: hex_color!("#27282E"),
                grey: hex_color!("#EBECF0"),
                white: hex_color!("#FFFFFF"),
                red: hex_color!("#FF6B68"),
                green: hex_color!("#A5C261"),
                yellow: hex_color!("#CC7832"),
                blue: hex_color!("#6897BB"),
                magenta: hex_color!("#9876AA"),
                cyan: hex_color!("#A9B7C6"),
            },
            light_prefs: Preferences {
                primary: Palette::Blue,
                secondary: Palette::Green,
                tertiary: Palette::Yellow,
                quaternary: Palette::Cyan,
            },
            code: None,
        }
    }

    pub fn vscode(current: Mode) -> Self {
        Self {
            current,
            dim: ThemeVariant {
                black: hex_color!("#1E1E1E"),
                grey: hex_color!("#333333"),
                white: hex_color!("#ffffff"),
                red: hex_color!("#D0372D"),
                green: hex_color!("#377E22"),
                yellow: hex_color!("#6F5529"),
                blue: hex_color!("#0000F5"),
                magenta: hex_color!("#96261F"),
                cyan: hex_color!("#3478C6"),
            },
            dark_prefs: Preferences {
                primary: Palette::Cyan,
                secondary: Palette::Blue,
                tertiary: Palette::Green,
                quaternary: Palette::Magenta,
            },
            bright: ThemeVariant {
                black: hex_color!("#000000"),
                grey: hex_color!("#F3F3F3"),
                white: hex_color!("#FFFFFF"),
                red: hex_color!("#D0372D"),
                green: hex_color!("#6BA456"),
                yellow: hex_color!("#DCDCAF"),
                blue: hex_color!("#679BD1"),
                magenta: hex_color!("#C5947C"),
                cyan: hex_color!("#3478C6"),
            },
            light_prefs: Preferences {
                primary: Palette::Cyan,
                secondary: Palette::Blue,
                tertiary: Palette::Green,
                quaternary: Palette::Magenta,
            },
            code: None,
        }
    }

    pub fn catppuccin(current: Mode) -> Self {
        Self {
            current,
            bright: ThemeVariant {
                black: hex_color!("#4c4f69"),
                grey: hex_color!("#e6e9ef"),
                white: hex_color!("#eff1f5"),
                red: hex_color!("#f38ba8"),
                green: hex_color!("#a6e3a1"),
                yellow: hex_color!("#f9e2af"),
                blue: hex_color!("#89b4fa"),
                magenta: hex_color!("#eba0ac"),
                cyan: hex_color!("#94e2d5"),
            },
            dark_prefs: Preferences {
                primary: Palette::Red,
                secondary: Palette::Blue,
                tertiary: Palette::Green,
                quaternary: Palette::Yellow,
            },
            dim: ThemeVariant {
                black: hex_color!("#11111b"),
                grey: hex_color!("#1e1e2e"),
                white: hex_color!("#cdd6f4"),
                red: hex_color!("#d20f39"),
                green: hex_color!("#40a02b"),
                yellow: hex_color!("#df8e1d"),
                blue: hex_color!("#1e66f5"),
                magenta: hex_color!("#e64553"),
                cyan: hex_color!("#179299"),
            },
            light_prefs: Preferences {
                primary: Palette::Red,
                secondary: Palette::Blue,
                tertiary: Palette::Green,
                quaternary: Palette::Yellow,
            },
            code: None,
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
            selection: {
                let accent = self.fg().get_color(self.prefs().primary);
                let paper = self.neutral_bg();
                style::Selection {
                    bg_fill: paper.lerp_to_gamma(accent, 0.18),
                    stroke: Stroke::new(1.0, paper.lerp_to_gamma(accent, 0.45)),
                }
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

        // Hover/press are programmatic ink washes on the resting control plate
        // (tertiary), not a jump to `neutral()` hairline grey.
        let control_rest = self.neutral_bg_tertiary();
        base.widgets.hovered.bg_fill = self.wash_toward_neutral_fg(control_rest, Self::HOVER_WASH);
        base.widgets.hovered.weak_bg_fill =
            self.wash_toward_neutral_fg(control_rest, Self::HOVER_WASH);
        base.widgets.hovered.fg_stroke.color = self.neutral_fg();
        base.widgets.hovered.bg_stroke.color = self.neutral();

        base.widgets.active.bg_fill = self.wash_toward_neutral_fg(control_rest, Self::PRESS_WASH);
        base.widgets.active.weak_bg_fill =
            self.wash_toward_neutral_fg(control_rest, Self::PRESS_WASH);
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Composite `src` over `dst` the way egui blends premultiplied colors
    /// in gamma space: `out = src_premul + (1 − src_a)·dst`.
    fn over(src: Color32, dst: Color32) -> [u8; 3] {
        let sa = src.a() as f32 / 255.0;
        let f = |s: u8, d: u8| (s as f32 + (1.0 - sa) * d as f32).round() as u8;
        [f(src.r(), dst.r()), f(src.g(), dst.g()), f(src.b(), dst.b())]
    }

    /// `translucent_over` must reproduce `target` when composited back over
    /// `background` — verified for the actual code-pill colors in both
    /// themes so clamping never silently shifts the unselected look.
    #[test]
    fn translucent_over_round_trips_code_pill() {
        for (label, mode) in [("light", Mode::Light), ("dark", Mode::Dark)] {
            let theme = Theme::default(mode);
            let target = theme.neutral_bg_secondary();
            let bg = theme.neutral_bg();
            let x = translucent_over(target, bg, 0.5);
            let got = over(x, bg);
            for (g, t) in got.iter().zip([target.r(), target.g(), target.b()]) {
                assert!(
                    (*g as i32 - t as i32).abs() <= 2,
                    "{label}: composite {got:?} != target {:?}",
                    [target.r(), target.g(), target.b()]
                );
            }
        }
    }

    #[test]
    fn contrast_ratio_classifies_wcag_thresholds() {
        assert_eq!(contrast_ratio(Color32::BLACK, Color32::WHITE), ContrastRatio::Maximum);
        assert_eq!(contrast_ratio(Color32::WHITE, Color32::WHITE), ContrastRatio::Low);
    }

    #[test]
    fn apple_theme_accent_passes_inline_code_contrast() {
        for mode in [Mode::Light, Mode::Dark] {
            let theme = Theme::apple(mode);
            let accent = theme.fg().get_color(theme.prefs().primary);
            let background = theme.neutral_bg_secondary();
            assert!(contrast_ratio(accent, background).passes_normal_text());
        }
    }

    #[test]
    fn system_accents_adjust_to_pass_normal_text() {
        let light_accents = [
            hex_color!("#FF383C"),
            hex_color!("#FF8D28"),
            hex_color!("#FFCC00"),
            hex_color!("#34C759"),
            hex_color!("#0088FF"),
            hex_color!("#CB30E0"),
            hex_color!("#FF2D55"),
            hex_color!("#8E8E93"),
        ];
        let dark_accents = [
            hex_color!("#FF4245"),
            hex_color!("#FF9230"),
            hex_color!("#FFD600"),
            hex_color!("#30D158"),
            hex_color!("#0091FF"),
            hex_color!("#DB34F2"),
            hex_color!("#FF375F"),
            hex_color!("#8E8E93"),
        ];

        let theme = Theme::apple(Mode::Light);
        for accent in light_accents {
            let adjusted = ensure_normal_text_contrast(accent, theme.bright.grey);
            assert!(contrast_ratio(adjusted, theme.bright.grey).passes_normal_text());
        }
        for accent in dark_accents {
            let adjusted = ensure_normal_text_contrast(accent, theme.dim.grey);
            assert!(contrast_ratio(adjusted, theme.dim.grey).passes_normal_text());
        }

        assert_eq!(ensure_normal_text_contrast(theme.dim.blue, theme.bright.grey), theme.dim.blue);
        assert_eq!(
            ensure_normal_text_contrast(theme.bright.blue, theme.dim.grey),
            theme.bright.blue
        );
    }
}
