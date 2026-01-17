use egui::Color32;
use epaint::hex_color;

pub struct Theme {
    current: Mode,
    light: ThemeVariant,
    dark: ThemeVariant,
}

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
