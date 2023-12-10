pub mod background;
pub mod syncing;
pub mod tab;
pub mod theme;
pub mod widgets;
pub mod workspace;

use epaint::text::FontDefinitions;

pub fn register_fonts(fonts: &mut FontDefinitions) {
    lbeditor::register_fonts(fonts)
}
