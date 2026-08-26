//! Design install: theme, fonts, zero `item_spacing`, type scale, hover time.
//! Mode preference defaults to system; pin via [`set_mode_preference`].

use egui::{Context, FontDefinitions, Id, Theme as EguiTheme, Vec2};
use workspace_rs::theme::palette_v2::{Mode, Theme, ThemeExt};

use super::chrome::HOVER_ANIM_SECS;
use super::typography;

const MODE_PREF_ID: &str = "design_mode_preference";
const THEME_FAMILY_ID: &str = "design_theme_family";
const DETECTED_MODE_ID: &str = "design_detected_system_mode";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum ModePreference {
    #[default]
    System,
    Light,
    Dark,
}

/// Built-in palette poles. Light/dark mode is separate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Hash)]
pub enum ThemeFamily {
    #[default]
    Default,
    Apple,
    Darcula,
    IntelliJ,
    VsCode,
    Catppuccin,
}

impl ThemeFamily {
    pub const ALL: [ThemeFamily; 6] =
        [Self::Default, Self::Apple, Self::Darcula, Self::IntelliJ, Self::VsCode, Self::Catppuccin];

    pub const fn name(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Apple => "apple",
            Self::Darcula => "darcula",
            Self::IntelliJ => "intellij",
            Self::VsCode => "vscode",
            Self::Catppuccin => "catppuccin",
        }
    }

    pub fn build(self, mode: Mode) -> Theme {
        match self {
            Self::Default => Theme::default(mode),
            Self::Apple => Theme::apple(mode),
            Self::Darcula => Theme::darcula(mode),
            Self::IntelliJ => Theme::intellij(mode),
            Self::VsCode => Theme::vscode(mode),
            Self::Catppuccin => Theme::catppuccin(mode),
        }
    }
}

/// Apply theme, fonts, spacing, type, and animation baseline.
///
/// Style is installed once per context (or after theme swap). Cloning and
/// `set_style` every frame is measurable UI-thread cost for no gain.
pub fn install(ctx: &Context) {
    ensure_theme(ctx);
    ensure_fonts(ctx);
    ensure_style(ctx);
    ensure_image_loaders(ctx);
}

/// Classic `Lockbook::deferred_init` installs these; shell must too for
/// `Image::from_bytes` (account QR) and other URI images.
fn ensure_image_loaders(ctx: &Context) {
    let id = Id::new("design_image_loaders_installed");
    if ctx.data(|d| d.get_temp::<()>(id).is_some()) {
        return;
    }
    egui_extras::install_image_loaders(ctx);
    ctx.data_mut(|d| d.insert_temp(id, ()));
}

fn ensure_style(ctx: &Context) {
    let id = Id::new("design_style_installed");
    if ctx.data(|d| d.get_temp::<()>(id).is_some()) {
        return;
    }
    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = Vec2::ZERO;
    style.animation_time = HOVER_ANIM_SECS;
    typography::install_text_styles(&mut style);
    ctx.set_style(style);
    ctx.data_mut(|d| d.insert_temp(id, ()));
}

/// Drop the style latch so the next [`install`] reapplies type / spacing
/// (call after theme family changes that reset visuals).
pub fn reinstall_style(ctx: &Context) {
    ctx.data_mut(|d| d.remove_temp::<()>(Id::new("design_style_installed")));
    ensure_style(ctx);
}

/// Install workspace fonts once (includes Phosphor).
fn ensure_fonts(ctx: &Context) {
    let id = Id::new("design_fonts_installed");
    if ctx.data(|d| d.get_temp::<()>(id).is_some()) {
        return;
    }
    let mut fonts = FontDefinitions::default();
    workspace_rs::register_fonts(&mut fonts);
    ctx.set_fonts(fonts);
    ctx.data_mut(|d| d.insert_temp(id, ()));
}

/// Ensure a theme exists; sync from OS when preference is system.
pub fn ensure_theme(ctx: &Context) {
    let has = ctx.memory(|m| m.data.get_temp::<Theme>(egui::Id::new("theme")).is_some());
    if !has {
        set_mode_preference(ctx, ModePreference::System);
        let family = theme_family(ctx);
        ctx.set_lb_theme(family.build(resolved_mode(ctx)));
        return;
    }
    sync_mode_from_preference(ctx);
}

pub fn theme_family(ctx: &Context) -> ThemeFamily {
    ctx.data(|d| {
        d.get_temp::<ThemeFamily>(Id::new(THEME_FAMILY_ID))
            .unwrap_or_default()
    })
}

pub fn set_theme_family(ctx: &Context, family: ThemeFamily) {
    ctx.data_mut(|d| d.insert_temp(Id::new(THEME_FAMILY_ID), family));
    ctx.set_lb_theme(family.build(resolved_mode(ctx)));
}

/// Current mode preference (system vs pinned).
pub fn mode_preference(ctx: &Context) -> ModePreference {
    ctx.data(|d| {
        d.get_temp::<ModePreference>(Id::new(MODE_PREF_ID))
            .unwrap_or(ModePreference::System)
    })
}

pub fn set_mode_preference(ctx: &Context, pref: ModePreference) {
    ctx.data_mut(|d| d.insert_temp(Id::new(MODE_PREF_ID), pref));
}

/// Resolved light/dark for painting (preference + OS).
pub fn resolved_mode(ctx: &Context) -> Mode {
    match mode_preference(ctx) {
        ModePreference::System => system_mode(ctx),
        ModePreference::Light => Mode::Light,
        ModePreference::Dark => Mode::Dark,
    }
}

/// OS appearance: egui `system_theme`, else one cached `dark_light` probe.
///
/// `dark_light::detect()` blocks on a portal / registry read — call it once.
pub fn system_mode(ctx: &Context) -> Mode {
    if let Some(theme) = ctx.system_theme() {
        return egui_theme_to_mode(theme);
    }
    if let Some(theme) = ctx.input(|i| i.raw.system_theme) {
        return egui_theme_to_mode(theme);
    }
    if let Some(cached) = ctx.data(|d| d.get_temp::<Mode>(Id::new(DETECTED_MODE_ID))) {
        return cached;
    }
    let mode = match dark_light::detect() {
        Ok(dark_light::Mode::Dark) => Mode::Dark,
        Ok(dark_light::Mode::Light | dark_light::Mode::Unspecified) | Err(_) => Mode::Light,
    };
    ctx.data_mut(|d| d.insert_temp(Id::new(DETECTED_MODE_ID), mode));
    mode
}

fn egui_theme_to_mode(theme: EguiTheme) -> Mode {
    match theme {
        EguiTheme::Dark => Mode::Dark,
        EguiTheme::Light => Mode::Light,
    }
}

/// Apply preference → Lockbook theme mode when it differs.
fn sync_mode_from_preference(ctx: &Context) {
    let want = resolved_mode(ctx);
    let has = ctx.memory(|m| m.data.get_temp::<Theme>(egui::Id::new("theme")).is_some());
    if !has {
        ctx.set_lb_theme(theme_family(ctx).build(want));
        return;
    }
    let current = ctx.get_lb_theme().current;
    if current != want {
        apply_mode(ctx, want);
    }
}

fn apply_mode(ctx: &Context, mode: Mode) {
    ctx.set_lb_theme(theme_family(ctx).build(mode));
    reinstall_style(ctx);
}
