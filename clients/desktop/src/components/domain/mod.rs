//! App-shaped surfaces that still belong in the component library
//! (multi-caller product chrome — sticky trees, pins, tabs, sync footer, …).

pub mod chips;
pub mod footer;
pub mod pins;
pub mod settings_plate;
pub mod sidebar_resize;
pub mod sync_dots;
pub mod tabs;
pub mod tree;

pub use footer::show as show_sync_footer;
pub use tree::{show_recents, show_shared, show_tree};
