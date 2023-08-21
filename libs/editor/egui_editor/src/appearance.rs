use std::collections::HashSet;

use egui::ecolor::Hsva;
use egui::{Color32, Visuals};

use crate::style::{BlockNodeType, InlineNodeType, MarkdownNodeType};

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
pub const BLACK: ThemedColor =
    ThemedColor { light: Color32::from_rgb(18, 18, 18), dark: Color32::from_rgb(240, 240, 240) };
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
pub const WHITE: ThemedColor =
    ThemedColor { light: Color32::from_rgb(240, 240, 240), dark: Color32::from_rgb(18, 18, 18) };

/// provides a mechanism for the application developer to override colors for dark mode and light
/// mode and for us to provide defaults
#[derive(Default)]
pub struct Appearance {
    pub current_theme: Theme,

    // colors
    pub text: Option<ThemedColor>,
    pub cursor: Option<ThemedColor>,
    pub selection_bg: Option<ThemedColor>,
    pub checkbox_bg: Option<ThemedColor>,
    pub heading: Option<ThemedColor>,
    pub heading_line: Option<ThemedColor>,
    pub code: Option<ThemedColor>,
    pub bold: Option<ThemedColor>,
    pub italics: Option<ThemedColor>,
    pub strikethrough: Option<ThemedColor>,
    pub link: Option<ThemedColor>,
    pub syntax: Option<ThemedColor>,

    // sizes
    pub bullet_radius: Option<f32>,
    pub checkbox_dim: Option<f32>,
    pub checkbox_rounding: Option<f32>,
    pub checkbox_slash_width: Option<f32>,
    pub rule_height: Option<f32>,
    pub image_padding: Option<f32>,

    // capture of markdown syntax characters
    pub markdown_capture: Option<HashSet<MarkdownNodeType>>,
}

impl Appearance {
    pub fn set_theme(&mut self, visuals: &Visuals) -> bool {
        let target_theme = if visuals.dark_mode { Theme::Dark } else { Theme::Light };

        if self.current_theme != target_theme {
            self.current_theme = target_theme;
            true
        } else {
            false
        }
    }

    pub fn text(&self) -> Color32 {
        self.text.unwrap_or(BLACK).get(self.current_theme)
    }

    pub fn cursor(&self) -> Color32 {
        self.cursor.unwrap_or(BLUE).get(self.current_theme)
    }

    pub fn selection_bg(&self) -> Color32 {
        let mut color = BLUE;

        color.light = {
            let mut color_hsva = Hsva::from(color.light);
            color_hsva.s /= 2.0;
            Color32::from(color_hsva)
        };
        color.dark = {
            let mut color_hsva = Hsva::from(color.dark);
            color_hsva.a /= 10.0;
            Color32::from(color_hsva)
        };

        self.selection_bg.unwrap_or(color).get(self.current_theme)
    }

    pub fn checkbox_bg(&self) -> Color32 {
        self.checkbox_bg.unwrap_or(GRAY_4).get(self.current_theme)
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
        self.strikethrough.unwrap_or(BLACK).get(self.current_theme)
    }

    pub fn link(&self) -> Color32 {
        self.link.unwrap_or(BLUE).get(self.current_theme)
    }

    pub fn syntax(&self) -> Color32 {
        self.syntax.unwrap_or(GRAY).get(self.current_theme)
    }

    pub fn bullet_radius(&self) -> f32 {
        self.bullet_radius.unwrap_or(2.5)
    }

    pub fn checkbox_dim(&self, touch_mode: bool) -> f32 {
        self.checkbox_dim
            .unwrap_or(if touch_mode { 16.0 } else { 12.0 })
    }

    pub fn checkbox_rounding(&self) -> f32 {
        self.checkbox_dim.unwrap_or(1.0)
    }

    pub fn checkbox_slash_width(&self) -> f32 {
        self.checkbox_dim.unwrap_or(2.0)
    }

    pub fn rule_height(&self) -> f32 {
        self.rule_height.unwrap_or(10.0)
    }

    pub fn image_padding(&self) -> f32 {
        self.image_padding.unwrap_or(12.0)
    }

    pub fn markdown_capture(&self, node_type: MarkdownNodeType) -> CaptureCondition {
        match node_type {
            MarkdownNodeType::Block(BlockNodeType::ListItem(_)) => CaptureCondition::Always,
            MarkdownNodeType::Block(BlockNodeType::Heading(_))
            | MarkdownNodeType::Block(BlockNodeType::Quote)
            | MarkdownNodeType::Block(BlockNodeType::Code)
            | MarkdownNodeType::Block(BlockNodeType::Rule)
            | MarkdownNodeType::Inline(InlineNodeType::Code)
            | MarkdownNodeType::Inline(InlineNodeType::Bold)
            | MarkdownNodeType::Inline(InlineNodeType::Italic)
            | MarkdownNodeType::Inline(InlineNodeType::Strikethrough)
            | MarkdownNodeType::Inline(InlineNodeType::Link)
            | MarkdownNodeType::Inline(InlineNodeType::Image) => CaptureCondition::NoCursor,
            MarkdownNodeType::Document | MarkdownNodeType::Paragraph => CaptureCondition::Never,
        }
    }
}

pub enum CaptureCondition {
    Always,
    NoCursor,
    Never,
}

#[derive(Default, Clone, Copy, PartialEq, Eq)]
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
