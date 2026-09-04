//! Shared chrome for sidebar lists (Recents, Shared, similar stacks).
//!
//! **Contract:** outer pad + section headers + zero vertical gap between rows.
//! Section breaks use [`Space::Sm`]; header air uses [`Space::Xs`].
//! Prefer this over ad-hoc `with_pad` so panes stay consistent.

use egui::{FontFamily, FontId, Pos2, Ui};

use crate::components::foundation::color::Theme;
use crate::components::foundation::space::Space;
use crate::components::foundation::typography::TypeRole;

/// Horizontal (+ vertical) inset for scrollable list bodies on canvas.
///
/// Files tree: equal L/R via [`crate::components::foundation::layout::with_h_pad`] + Spacers (not paint
/// offsets). Recents / Shared: same pad around the list body.
pub const LIST_PAD: Space = Space::Sm;

/// Air above a section header when following another section.
pub const SECTION_GAP: Space = Space::Sm;

/// Air between section title and first row.
pub const SECTION_HEAD_GAP: Space = Space::Xs;

/// Virtualized section label (Recents buckets / Shared sharer groups).
pub fn paint_list_section(ui: &Ui, t: &Theme, title: &str, pos: Pos2) {
    let g = ui.painter().layout_no_wrap(
        title.to_owned(),
        FontId::new(TypeRole::Body.size(), FontFamily::Name(std::sync::Arc::from("Bold"))),
        t.neutral_fg_secondary(),
    );
    ui.painter().galley(pos, g, t.neutral_fg_secondary());
}
