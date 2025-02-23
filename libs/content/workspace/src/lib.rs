pub mod file_cache;
pub mod mind_map;
pub mod output;
pub mod show;
pub mod syncing;
pub mod tab;
pub mod task_manager;
pub mod theme;
pub mod widgets;
pub mod workspace;

pub use output::Response;
pub use tab::Event;

use epaint::text::FontDefinitions;

pub fn register_fonts(fonts: &mut FontDefinitions) {
    tab::markdown_editor::register_fonts(fonts)
}
