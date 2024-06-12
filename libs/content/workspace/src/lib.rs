pub mod background;
pub mod output;
pub mod syncing;
pub mod tab;
pub mod theme;
pub mod widgets;
pub mod workspace;

pub use tab::Event;
pub use lb_rs;

use epaint::text::FontDefinitions;

pub fn register_fonts(fonts: &mut FontDefinitions) {
    tab::markdown_editor::register_fonts(fonts)
}
