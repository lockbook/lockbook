//! A file name with, when another visible item has the same name, the folder
//! that tells it apart, dimmed after it. Pin chips and tabs share the rule
//! that picks the folder and the layout that fits name and folder in a slot.

use egui::{Ui, pos2, vec2};

use crate::components::foundation::color::Theme;
use crate::components::foundation::space::Space;
use crate::components::foundation::typography::TypeRole;
use workspace_rs::style::file_name::{self, measure as measure_file_name, paint_body};
use workspace_rs::style::file_row::CRUMB_SEP;
use workspace_rs::style::{crumb_label, home_crumb};

/// Gap between the name and its folder.
pub const GAP: Space = Space::Xs;

/// A folder path's crumbs, outermost first; own-tree paths start at Home.
pub fn folder_crumbs(path: &str) -> Vec<String> {
    let home = path.starts_with('/').then(home_crumb);
    home.into_iter()
        .chain(path.split('/').filter(|p| !p.is_empty()).map(str::to_owned))
        .collect()
}

/// For each `(name, folder crumbs)` item, the fewest innermost crumbs that
/// set it apart from every other item of the same name; `None` when no
/// same-named item lives in another folder.
pub fn disambiguating_folders(items: &[(&str, Vec<String>)]) -> Vec<Option<String>> {
    let trail = |crumbs: &[String], k: usize| crumbs[crumbs.len().saturating_sub(k)..].to_vec();
    items
        .iter()
        .enumerate()
        .map(|(i, (name, crumbs))| {
            let twins: Vec<&Vec<String>> = items
                .iter()
                .enumerate()
                .filter(|(j, (other, c))| *j != i && other == name && c != crumbs)
                .map(|(_, (_, c))| c)
                .collect();
            if twins.is_empty() {
                return None;
            }
            let k = (1..=crumbs.len())
                .find(|&k| twins.iter().all(|c| trail(c, k) != trail(crumbs, k)))
                .unwrap_or(crumbs.len());
            Some(trail(crumbs, k).join(CRUMB_SEP))
        })
        .collect()
}

/// Width of the name and its folder, untruncated. `name` is the display name.
pub fn natural_width(ui: &Ui, name: &str, folder: Option<&str>) -> f32 {
    let name_w = measure_file_name(ui, name);
    match folder {
        None => name_w,
        Some(folder) => name_w + GAP.pts() + measure_file_name(ui, folder),
    }
}

/// Paints `name` (the display name) and its folder on one body line starting
/// at `line.left()`, within `line.width()`. The name yields to the folder down
/// to half the slot; the folder then shortens from the inside.
pub fn paint(ui: &mut Ui, t: &Theme, name: &str, folder: Option<&str>, line: egui::Rect) {
    let slot_w = line.width().max(8.0);
    let gap = GAP.pts();
    let lh = TypeRole::Body.line_height();
    let (name_w, folder) = match folder {
        None => (slot_w, None),
        Some(folder) => {
            let measure = |s: &str| measure_file_name(ui, s);
            let name_w = measure(name).min((slot_w - gap - measure(folder)).max(slot_w / 2.0));
            (name_w, Some(fit_trail(folder, slot_w - name_w - gap, measure)))
        }
    };
    let drawn =
        paint_body(ui, name, t.neutral_fg(), egui::Rect::from_min_size(line.min, vec2(name_w, lh)));
    if let Some(folder) = folder {
        let x = line.left() + drawn + gap;
        file_name::paint_label(
            ui,
            crumb_label(&folder, t.neutral_fg_secondary(), t.neutral_fg_secondary()),
            egui::Rect::from_min_size(pos2(x, line.top()), vec2(line.right() - x, lh)),
            file_name::body_font_size(),
            lh,
        );
    }
}

/// Shortens a folder trail from the inside, always keeping its outermost
/// crumb (the one that tells same-named items apart): `a/b/c` → `a/…/c` → `a/…`.
fn fit_trail(trail: &str, max_w: f32, measure: impl Fn(&str) -> f32) -> String {
    let parts: Vec<&str> = trail.split(CRUMB_SEP).collect();
    if measure(trail) <= max_w || parts.len() < 2 {
        return trail.to_owned();
    }
    (0..parts.len() - 1)
        .rev()
        .map(|inner| {
            let mut kept = vec![parts[0], "…"];
            kept.extend(&parts[parts.len() - inner..]);
            kept.join(CRUMB_SEP)
        })
        .find(|candidate| measure(candidate) <= max_w)
        .unwrap_or_else(|| parts[0].to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item<'a>(name: &'a str, folder: &str) -> (&'a str, Vec<String>) {
        (name, folder_crumbs(folder))
    }

    #[test]
    fn same_named_items_walk_up_until_they_differ() {
        let items = [
            item("todo", "/"),
            item("todo", "/work/"),
            item("notes", "/a/x/y/"),
            item("notes", "/b/x/y/"),
            item("plan", "/work/"),
            item("plan", "/work/"), // plan.md beside plan.pdf
        ];
        assert_eq!(
            disambiguating_folders(&items),
            [
                Some(home_crumb()),
                Some("work".into()),
                Some("a/x/y".into()),
                Some("b/x/y".into()),
                None,
                None,
            ]
        );
    }
}
