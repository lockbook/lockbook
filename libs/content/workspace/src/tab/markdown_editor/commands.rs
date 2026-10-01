//! Editor actions a platform delivers as commands instead of pointer
//! events: taps on its touch targets, decided by the platform's own gesture system.

use egui::Pos2;
use lb_rs::model::text::offset_types::Grapheme;

use super::input::Event;
use super::{MdEdit, TouchTarget};

impl MdEdit {
    /// The touch target painted under `pos` last frame.
    pub fn touch_target_at(&self, pos: Pos2) -> Option<&TouchTarget> {
        self.renderer
            .touch_targets
            .iter()
            .find(|(rect, _)| rect.contains(pos))
            .map(|(_, target)| target)
    }

    /// Tap the target under `pos`. Its event is queued as a click's would
    /// be, except a selection, which is the platform's to write: that range
    /// is returned instead.
    pub fn tap(&mut self, pos: Pos2) -> Option<(Grapheme, Grapheme)> {
        let Some(TouchTarget::Tap(event)) = self.touch_target_at(pos).cloned() else {
            return None;
        };
        self.renderer.ctx.request_repaint();
        match event {
            Event::Select { region } => Some(self.region_to_range(region)),
            event => {
                self.renderer.render_events.push(event);
                None
            }
        }
    }
}
