use crate::tab::{ExtendedInput as _, markdown_editor};
use egui::Context;
use markdown_editor::MdEdit;
use markdown_editor::input::Event;

impl MdEdit {
    /// Drain `Markdown` / `Undo` / `Redo` events from the egui context. Drop
    /// and Paste are left for the workspace's image-import pass;
    /// `PredictedTouch` / `MultiTouchGesture` are left for other tabs. Lives on
    /// `MdEdit` so a standalone composer (chat) drains the same events the
    /// markdown editor does.
    pub(crate) fn drain_workspace_events(&self, ctx: &Context) -> Vec<Event> {
        if self.renderer.readonly {
            return Vec::new();
        }
        ctx.pop_events_where(&mut |e| {
            matches!(e, crate::Event::Markdown(_) | crate::Event::Undo | crate::Event::Redo)
        })
        .into_iter()
        .filter_map(|e| match e {
            crate::Event::Markdown(modification) => Some(modification),
            crate::Event::Undo => Some(Event::Undo),
            crate::Event::Redo => Some(Event::Redo),
            _ => None,
        })
        .collect()
    }
}
