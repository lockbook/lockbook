use egui::Color32;
use workspace_rs::theme::palette_v2::{Theme, ThemeExt};
use workspace_rs::widgets::FloatingChrome;

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
    /// One step above `surface` — elevated chrome that still sits in the panel
    /// (resting widget fills). Maps to `neutral_bg_tertiary`.
    pub fn surface_raised(&self) -> Color32 {
        self.theme.neutral_bg_tertiary()
    }
    /// Hairlines, borders, resting outlines.
    pub fn line(&self) -> Color32 {
        self.theme.neutral()
    }
    /// Primary ink.
    pub fn fg(&self) -> Color32 {
        self.theme.neutral_fg()
    }
    /// Secondary / de-emphasized text and icons (file-tree labels, nav ink).
    pub fn text_muted(&self) -> Color32 {
        self.theme.neutral_fg_secondary()
    }
    /// Faint captions and markers — same ramp step as `text_muted` on the
    /// original palette (no separate muted tier).
    pub fn text_faint(&self) -> Color32 {
        self.theme.neutral_fg_secondary()
    }
    /// Semantic accent (theme primary) — folder icons, active chrome. Matches
    /// Apple's `Color.accentColor` role for file-tree folders.
    pub fn accent(&self) -> Color32 {
        self.theme.fg().get_color(self.theme.prefs().primary)
    }
    /// Foreground red for destructive text/outline — follows the fg ramp, so it
    /// lightens in dark mode (the bright-variant red) to stay readable.
    pub fn danger(&self) -> Color32 {
        self.theme.fg().red
    }

    /// Floating chrome (menus, tips, hover cards) — canvas + line.
    pub fn floating(&self) -> FloatingChrome {
        FloatingChrome::new(self.canvas(), self.line())
    }
}
