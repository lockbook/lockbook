use egui::Color32;
use workspace_rs::theme::palette_v2::{Theme, ThemeExt};

/// A typed view over the active `palette_v2` theme — the neutral spine plus the
/// one semantic accent (danger). It holds the `Theme`, not a snapshot of its
/// resolved colors, so every accessor reflects the current palette and a
/// theme/mode change is picked up the moment it lands. Cheap to rebuild (`Theme`
/// is `Copy`); make one per frame.
pub struct Tokens {
    theme: Theme,
}

impl Tokens {
    pub fn new(ctx: &egui::Context) -> Self {
        Self { theme: ctx.get_lb_theme() }
    }

    /// Editable-content background — white / near-black.
    pub fn canvas(&self) -> Color32 {
        self.theme.neutral_bg()
    }
    /// UI background one step off the canvas — panels, resting wireframe fill.
    pub fn surface(&self) -> Color32 {
        self.theme.neutral_bg_secondary()
    }
    /// Hairlines, borders, resting outlines.
    pub fn line(&self) -> Color32 {
        self.theme.neutral()
    }
    /// Primary ink.
    pub fn fg(&self) -> Color32 {
        self.theme.neutral_fg()
    }
    /// Secondary text that still reads easily.
    pub fn text_muted(&self) -> Color32 {
        self.theme.neutral_fg_muted()
    }
    /// Faint captions and markers.
    pub fn text_faint(&self) -> Color32 {
        self.theme.neutral_fg_secondary()
    }
    /// Foreground red for destructive text/outline — follows the fg ramp, so it
    /// lightens in dark mode (the bright-variant red) to stay readable.
    pub fn danger(&self) -> Color32 {
        self.theme.fg().red
    }
}
