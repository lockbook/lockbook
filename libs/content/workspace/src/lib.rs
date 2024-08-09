pub mod background;
pub mod output;
mod status;
pub mod syncing;
pub mod tab;
pub mod theme;
pub mod widgets;
pub mod workspace;

pub use output::Response;
pub use tab::Event;

use epaint::text::FontDefinitions;

pub fn register_fonts(fonts: &mut FontDefinitions) {
    tab::markdown_editor::register_fonts(fonts)
}
