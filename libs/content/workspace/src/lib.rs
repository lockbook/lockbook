pub(crate) mod egress;
pub mod file_cache;
pub mod font;
pub mod history_swipe;
pub mod landing;
pub mod link_plan;
pub mod link_preview;
pub mod links;
#[cfg(not(target_family = "wasm"))]
pub mod mind_map;
pub mod note_index;
pub mod open_guard;
pub mod output;
pub mod resolvers;
pub mod search;
pub mod seq;
pub mod show;
pub mod space_inspector;
pub mod style;
pub mod tab;
pub mod task_manager;
pub mod theme;
pub mod widgets;
pub mod workspace;

#[cfg(test)]
mod file_cache_participant_tests;
#[cfg(test)]
mod file_cache_prop_tests;
#[cfg(test)]
mod test_utils;

pub use output::Response;
pub use tab::Event;

pub fn register_fonts(fonts: &mut epaint::text::FontDefinitions) {
    tab::markdown_editor::register_fonts(fonts)
}

pub use widgets::glyphon_render::{
    GlyphonRenderCallbackResources, GlyphonRendererCallback, TextBufferArea, register_font_system,
    register_render_callback_resources,
};
