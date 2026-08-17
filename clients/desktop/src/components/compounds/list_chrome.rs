//! Shared chrome for sidebar lists (Recents, Shared, similar stacks).
//!
//! **Contract:** outer pad + section headers + zero vertical gap between rows.
//! Section breaks use [`Space::Sm`]; header air uses [`Space::Xs`].
//! Prefer this over ad-hoc `with_pad` so panes stay consistent.

use egui::Ui;

use crate::components::foundation::color::Theme;
use crate::components::foundation::space::Space;
use crate::components::foundation::spacer::Spacer;
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

/// Age / group label in list bodies (Recents buckets, Shared groups).
///
/// Proportional muted body — not mono settings eyebrows ([`super::form::section_label`]).
pub fn list_section_header(ui: &mut Ui, t: &Theme, title: &str) {
    ui.label(
        TypeRole::Body
            .rich(title)
            .strong()
            .color(t.neutral_fg_secondary()),
    );
    ui.add(Spacer::new(SECTION_HEAD_GAP));
}
