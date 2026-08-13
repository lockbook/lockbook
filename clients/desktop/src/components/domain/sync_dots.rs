//! Debounced per-file sync dots for the Files tree.
//!
//! - **2s** debounce before a dot appears
//! - Colors: green push / yellow dirty / blue pull
//! - Size: **8 pt** diameter
//! - Bubbles to the nearest **collapsed** ancestor so hidden dirty kids still read
//! - Priority: pushing > dirty > pulling

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use egui::{Color32, Context};
use lb::Uuid;
use lb::subscribers::status::Status;
use workspace_rs::file_cache::FilesExt;
use workspace_rs::theme::palette_v2::Palette;

use crate::components::Theme;

/// Debounce before a status dot appears.
const DEBOUNCE: Duration = Duration::from_secs(2);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyncDot {
    Pushing,
    Dirty,
    Pulling,
}

impl SyncDot {
    fn rank(self) -> u8 {
        match self {
            SyncDot::Pushing => 0,
            SyncDot::Dirty => 1,
            SyncDot::Pulling => 2,
        }
    }

    /// Mode-aware hue for push / dirty / pull.
    pub fn color(self, t: &Theme) -> Color32 {
        match self {
            SyncDot::Pushing => t.fg().get_color(Palette::Green),
            SyncDot::Dirty => t.fg().get_color(Palette::Yellow),
            SyncDot::Pulling => t.fg().get_color(Palette::Blue),
        }
    }
}

#[derive(Default)]
pub struct SyncDots {
    /// When each id first entered a status list (cleared when it leaves).
    pending_since: HashMap<Uuid, Instant>,
    /// After debounce: visible representative id → kind.
    visible: HashMap<Uuid, SyncDot>,
    pushing: Vec<Uuid>,
    dirty: Vec<Uuid>,
    pulling: Vec<Uuid>,
    expanded: HashSet<Uuid>,
}

impl SyncDots {
    /// Recompute from live status + expanded folders. Call once per frame before tree paint.
    pub fn refresh(
        &mut self, ctx: &Context, status: &Status, expanded: &HashSet<Uuid>, files: &impl FilesExt,
    ) {
        let inputs_unchanged = self.pushing == status.pushing_files
            && self.dirty == status.dirty_locally
            && self.pulling == status.pulling_files
            && self.expanded == *expanded;

        if !inputs_unchanged {
            let now = Instant::now();
            let mut pending_since = HashMap::new();
            for ids in [&status.pushing_files, &status.dirty_locally, &status.pulling_files] {
                for &id in ids {
                    let since = self.pending_since.get(&id).copied().unwrap_or(now);
                    pending_since.insert(id, since);
                }
            }
            self.pending_since = pending_since;
            self.pushing = status.pushing_files.clone();
            self.dirty = status.dirty_locally.clone();
            self.pulling = status.pulling_files.clone();
            self.expanded = expanded.clone();
        }

        self.visible = self.compute_visible(status, files);

        // Wake when the next debounce window ends so dots appear without pointer motion.
        let soonest = self
            .pending_since
            .values()
            .map(|since| DEBOUNCE.saturating_sub(since.elapsed()))
            .filter(|remaining| !remaining.is_zero())
            .min();
        if let Some(remaining) = soonest {
            ctx.request_repaint_after(remaining);
        }
    }

    pub fn color_for(&self, id: Uuid, t: &Theme) -> Option<Color32> {
        self.visible.get(&id).map(|d| d.color(t))
    }

    fn compute_visible(&self, status: &Status, files: &impl FilesExt) -> HashMap<Uuid, SyncDot> {
        let mut dots = HashMap::new();
        for (ids, kind) in [
            (&status.pushing_files, SyncDot::Pushing),
            (&status.dirty_locally, SyncDot::Dirty),
            (&status.pulling_files, SyncDot::Pulling),
        ] {
            for &id in ids {
                if !self.debounce_elapsed(id) {
                    continue;
                }
                let rep = visible_representative(id, files, &self.expanded);
                bump(&mut dots, rep, kind);
            }
        }
        dots
    }

    fn debounce_elapsed(&self, id: Uuid) -> bool {
        self.pending_since
            .get(&id)
            .is_some_and(|since| since.elapsed() >= DEBOUNCE)
    }
}

fn bump(dots: &mut HashMap<Uuid, SyncDot>, id: Uuid, kind: SyncDot) {
    let replace = dots
        .get(&id)
        .is_none_or(|existing| kind.rank() < existing.rank());
    if replace {
        dots.insert(id, kind);
    }
}

/// Walk up until root; each collapsed parent becomes the painted representative
/// Dirty descendant under a collapsed folder lights the folder.
fn visible_representative(id: Uuid, files: &impl FilesExt, expanded: &HashSet<Uuid>) -> Uuid {
    let mut target = id;
    let mut current = id;
    while let Some(file) = files.get_by_id(current) {
        if file.is_root() {
            break;
        }
        if !expanded.contains(&file.parent) {
            target = file.parent;
        }
        current = file.parent;
    }
    target
}
