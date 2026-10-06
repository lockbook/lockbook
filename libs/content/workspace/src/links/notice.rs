//! The notice link upkeep shows over the workspace: what a change did to
//! links in one line, and one action that sets it right.

use std::ops::Range;

use egui::os::OperatingSystem;
use egui::{Align, Area, Id, Label, Layout, Order, Rect, Sense, Ui, UiBuilder, pos2, vec2};
use lb_rs::Uuid;
use lb_rs::model::access_info::UserAccessMode;
use web_time::Duration;

use super::upkeep::{Mend, Stray};
use super::{LinkIndex, Notice};
use crate::file_cache::FilesExt;
use crate::style::chrome::canvas_overlay_frame;
use crate::style::{
    Button, FG_HOVER, Radius, Space, ThemeExt as _, TypeRole, control_height, display_file_name,
    icon_button, phosphor, sense_click,
};
use crate::tab::markdown_editor::TouchTarget;
use crate::workspace::Workspace;

const WIDTH: f32 = 440.0;
/// Rows listed before the list scrolls.
const ROWS: usize = 6;
/// Seconds between one notice or row being acted on and the next, in its
/// place, taking a click: a double click's second lands there.
const BEAT: f64 = 0.4;

/// One line of a notice's list: a file to open, and what happens to it.
#[derive(Debug, PartialEq, Eq)]
struct Row {
    file: Uuid,
    name: String,
    detail: String,
    /// Where in the file the link is, in bytes.
    at: Option<Range<usize>>,
    /// Whether the row can be acted on by itself.
    update: bool,
}

fn name<F: FilesExt + ?Sized>(files: &F, id: Uuid) -> String {
    files
        .get_by_id(id)
        .map_or("a file", |f| display_file_name(&f.name))
        .to_string()
}

/// A name short enough to sit in a sentence.
fn brief(name: &str) -> String {
    const MAX: usize = 32;
    match name.char_indices().nth(MAX) {
        Some((cut, _)) => format!("{}…", name[..cut].trim_end()),
        None => name.into(),
    }
}

fn count(n: usize, one: &str, many: &str) -> String {
    if n == 1 { one.into() } else { format!("{n} {many}") }
}

/// The one file every item is about, if there is just one.
fn only<T: PartialEq + Copy>(mut items: impl Iterator<Item = T>) -> Option<T> {
    let first = items.next()?;
    items.all(|item| item == first).then_some(first)
}

/// A notice's one line, and what its action is called.
fn headline<F: FilesExt + ?Sized>(
    notice: &Notice, files: &F, can_mend: &dyn Fn(Uuid) -> bool,
) -> (String, Option<&'static str>) {
    match notice {
        Notice::Strays(strays) => {
            let mendable = strays
                .iter()
                .filter(|s| s.mend.is_some() && can_mend(s.note));
            let n = mendable.clone().count();
            let (of, verb) = if n > 0 {
                (n, if n == 1 { "needs updating" } else { "need updating" })
            } else {
                (strays.len(), if strays.len() == 1 { "is broken" } else { "are broken" })
            };
            let to = match only(strays.iter().map(|s| s.meant)) {
                Some(file) => format!(" to {}", brief(&name(files, file))),
                None => String::new(),
            };
            let links = count(of, "A link", "links");
            (format!("{links}{to} {verb}"), (n > 0).then_some("Update"))
        }
        Notice::Orphans(orphans) => {
            let images = count(orphans.len(), "A pasted image is", "pasted images are");
            (format!("{images} no longer used"), Some("Delete"))
        }
        Notice::Lost(lost) => {
            let notes = count(lost.len(), &brief(&name(files, lost[0].0)), "notes");
            let verb = if lost.len() == 1 { "links" } else { "link" };
            let what = match only(lost.iter().map(|(_, file)| file.as_str())) {
                Some(file) => {
                    let file = brief(display_file_name(file));
                    format!("{notes} {verb} to {file}, which is gone")
                }
                None => format!("{notes} {verb} to files that are gone"),
            };
            (what, None)
        }
    }
}

fn rows<F: FilesExt + ?Sized>(
    notice: &Notice, files: &F, index: &LinkIndex, can_mend: &dyn Fn(Uuid) -> bool,
) -> Vec<Row> {
    let row = |file: Uuid, detail: String| Row {
        file,
        name: name(files, file),
        detail,
        at: None,
        update: false,
    };
    // destinations read as the paths they are
    let plain = |dest: &str| urlencoding::decode(dest).map_or(dest.to_string(), |d| d.into_owned());
    match notice {
        Notice::Strays(strays) => {
            let mendable = |s: &&Stray| s.mend.is_some() && can_mend(s.note);
            let several = strays.iter().filter(mendable).count() > 1;
            let listed = strays.iter().map(|stray| {
                let Stray { note, kind, dest, mend, .. } = stray;
                let written = |l: &&super::Indexed| l.link.kind == *kind && l.link.dest == *dest;
                let mut links = index.outbound(*note).iter().filter(written).peekable();
                let found = links.peek().is_some();
                let at = links.find_map(|l| Some(l.link.spans.clone()?.link));
                let dest = plain(dest);
                let detail = match mend {
                    _ if !can_mend(*note) => format!("{dest} · read-only"),
                    Some(Mend::Dest(new)) if !new.starts_with("lb://") => {
                        format!("{dest} → {}", plain(new))
                    }
                    Some(_) => format!("{dest} → a link by id"),
                    // written apart from its link, as a reference's is
                    None if found && at.is_none() => format!("{dest} · update by hand"),
                    None => format!("{dest} · outside what's shared"),
                };
                Row { at, update: several && mendable(&stray), ..row(*note, detail) }
            });
            listed.collect()
        }
        Notice::Orphans(orphans) => orphans
            .iter()
            .map(|file| {
                let folder = files.get_by_id(*file).map(|f| files.path(f.parent));
                row(*file, folder.unwrap_or_default())
            })
            .collect(),
        Notice::Lost(lost) => lost
            .iter()
            .map(|(note, file)| row(*note, format!("{} is gone", display_file_name(file))))
            .collect(),
    }
}

/// What the user did to a notice this frame.
#[derive(Default)]
struct Did {
    act: bool,
    dismiss: bool,
    toggle_list: bool,
    /// A row to open, and one to act on by itself.
    open: Option<usize>,
    update: Option<usize>,
}

/// The notice's plate, `width` wide: the headline with its controls beside
/// it, then the rows when `listing`.
fn show(
    ui: &mut Ui, width: f32, headline: &str, action: Option<&str>, rows: &[Row], listing: bool,
) -> Did {
    let t = ui.ctx().get_lb_theme();
    let ground = t.neutral_bg();
    let pad = Space::Sm;
    let mut did = Did::default();
    canvas_overlay_frame(&t, pad).show(ui, |ui| {
        ui.set_width(width - pad.pts() * 2.0);
        ui.spacing_mut().item_spacing = vec2(Space::Xs.pts(), Space::Xs.pts());

        // controls differ by notice; so must their ids, which carry animation
        ui.push_id(action, |ui| {
            ui.horizontal(|ui| {
                ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                    did.dismiss = icon_button(ui, &t, phosphor::X, false, ground).clicked();
                    if let Some(action) = action {
                        did.act = Button::primary(&t, action).show(ui).clicked();
                    }
                    let list = if listing { "Hide" } else { "Show" };
                    did.toggle_list = Button::quiet(&t, list).show(ui).clicked();
                    ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                        ui.set_min_height(control_height());
                        let text = TypeRole::Body.rich(headline).color(t.neutral_fg());
                        ui.add(Label::new(text).wrap().selectable(false));
                    });
                });
            })
        });
        if !listing {
            return;
        }

        let row_h = control_height();
        let shown = rows.len().min(ROWS) as f32;
        let (list, _) =
            ui.allocate_exact_size(vec2(ui.available_width(), row_h * shown), Sense::hover());
        let mut ui = ui.new_child(UiBuilder::new().max_rect(list));
        ui.spacing_mut().item_spacing.y = 0.0;
        egui::ScrollArea::vertical().auto_shrink(false).show_rows(
            &mut ui,
            row_h,
            rows.len(),
            |ui, shown| {
                for (i, row) in shown.clone().zip(&rows[shown]) {
                    let size = vec2(ui.available_width(), row_h);
                    let (rect, response) = ui.allocate_exact_size(size, sense_click());
                    if response.hovered() {
                        let wash = t.wash_toward_neutral_fg(ground, FG_HOVER);
                        ui.painter().rect_filled(rect, Radius::Sm.corner(), wash);
                    }
                    if response.clicked() {
                        did.open = Some(i);
                    }
                    let mut ui = ui.new_child(
                        UiBuilder::new()
                            .id_salt(i)
                            .max_rect(rect.shrink2(vec2(Space::Xs.pts(), 0.0)))
                            .layout(Layout::right_to_left(Align::Center)),
                    );
                    if row.update && Button::quiet(&t, "Update").show(&mut ui).clicked() {
                        did.update = Some(i);
                    }
                    ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                        let name = TypeRole::Body.rich(&row.name).color(t.neutral_fg());
                        ui.add(Label::new(name).selectable(false));
                        let detail = TypeRole::Body
                            .rich(&row.detail)
                            .color(t.neutral_fg_secondary());
                        ui.add(Label::new(detail).selectable(false).truncate());
                    });
                }
            },
        );
    });
    did
}

impl Workspace {
    /// Shows what link upkeep has for the user, floating over `bounds`.
    pub(crate) fn show_link_notice(&mut self, ui: &mut Ui, bounds: Rect) {
        let Some(notice) = self.link_notice().cloned() else {
            self.link_upkeep.listing = false;
            return;
        };
        let now = ui.input(|i| i.time);
        let wait = self.link_upkeep.acted + BEAT - now;
        if wait > 0.0 {
            ui.ctx()
                .request_repaint_after(Duration::from_secs_f64(wait));
            return;
        }
        let listing = self.link_upkeep.listing;
        let (headline, action, rows) = {
            let files = self.files.read().unwrap();
            let index = self.links.read().unwrap();
            let can_mend = |note| {
                files.get_by_id(note).is_some()
                    && files.access(note, &self.account) != UserAccessMode::Read
            };
            let (headline, action) = headline(&notice, &*files, &can_mend);
            let rows = if listing { rows(&notice, &*files, &index, &can_mend) } else { vec![] };
            (headline, action, rows)
        };
        let margin = Space::Md.pts();
        let width = WIDTH.min(bounds.width() - margin * 2.0);
        if width < WIDTH / 2.0 {
            return; // no room to say it
        }

        // laid out once where no pointer is, for its size
        let away = Rect::from_min_size(pos2(0.0, -10_000.0), vec2(width, 0.0));
        let measure = UiBuilder::new().id_salt("link_notice_size").max_rect(away);
        let mut measure = ui.new_child(measure.invisible());
        show(&mut measure, width, &headline, action, &rows, listing);
        let size = measure.min_rect().size();

        // clear of the keyboard and toolbar on a phone, of the text on a desktop
        let touch = matches!(ui.ctx().os(), OperatingSystem::Android | OperatingSystem::IOS);
        let top = if touch { bounds.top() } else { bounds.bottom() - margin * 2.0 - size.y };
        let top = top.max(bounds.top()) + margin;
        let area = Area::new(Id::new("link_notice"))
            .order(Order::Foreground)
            .fixed_pos(pos2(bounds.center().x - size.x / 2.0, top))
            .constrain(false)
            .show(ui.ctx(), |ui| show(ui, width, &headline, action, &rows, listing));

        // a platform that routes touches itself leaves this to egui
        if let Some(md) = self.current_tab_markdown_mut() {
            let target = (area.response.rect, TouchTarget::Popup);
            md.edit.renderer.touch_targets.push(target);
        }

        let did = area.inner;
        if did.toggle_list {
            self.link_upkeep.listing = !listing;
        }
        if let Some(row) = did.open.and_then(|i| rows.get(i)) {
            match row.at.clone() {
                Some(at) => self.open_file_at_range(row.file, at, false),
                None => self.open_file(row.file, true, false),
            }
        }
        match (did.update, notice) {
            _ if did.dismiss => self.dismiss_link_notice(),
            _ if did.act => self.accept_link_notice(),
            // the rows below slide up under the pointer, where a double
            // click's second would land
            (Some(i), Notice::Strays(mut strays))
                if i < strays.len() && now - self.link_upkeep.row_acted > BEAT =>
            {
                self.link_upkeep.row_acted = now;
                self.mend_links(vec![strays.swap_remove(i)])
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::links::LinkKind;
    use crate::test_utils::files::{at, tree};

    #[test]
    fn says_what_happened_and_what_it_will_do() {
        let files = tree("alice", &["/plan.md", "/notes.md", "/log.md", "/imports/chart.png"]);
        let (plan, notes, log) =
            (at(&files, "/plan.md"), at(&files, "/notes.md"), at(&files, "/log.md"));
        let chart = at(&files, "/imports/chart.png");
        let stray = |note, dest: &str, meant, mend| Stray {
            note,
            kind: LinkKind::Link,
            dest: dest.into(),
            meant,
            mend,
        };
        let writable = |_| true;
        let said = |notice: &Notice| headline(notice, &files, &writable);

        let one = stray(notes, "old.md", plan, Some(Mend::Dest("plan.md".into())));
        let other = stray(log, "old.md", plan, Some(Mend::Dest("plan.md".into())));
        let embed = stray(log, "chart.png", chart, None);
        assert_eq!(
            said(&Notice::Strays(vec![one.clone()])),
            ("A link to plan needs updating".into(), Some("Update"))
        );
        assert_eq!(
            said(&Notice::Strays(vec![one.clone(), other.clone()])),
            ("2 links to plan need updating".into(), Some("Update"))
        );
        assert_eq!(
            said(&Notice::Strays(vec![one.clone(), embed.clone()])),
            ("A link needs updating".into(), Some("Update"))
        );
        assert_eq!(
            said(&Notice::Strays(vec![embed.clone()])),
            ("A link to chart.png is broken".into(), None)
        );
        // a note that can't be written isn't updated
        assert_eq!(
            headline(&Notice::Strays(vec![one.clone()]), &files, &|_| false),
            ("A link to plan is broken".into(), None)
        );

        assert_eq!(
            said(&Notice::Orphans(vec![chart])),
            ("A pasted image is no longer used".into(), Some("Delete"))
        );
        assert_eq!(
            said(&Notice::Orphans(vec![chart, plan])),
            ("2 pasted images are no longer used".into(), Some("Delete"))
        );
        assert_eq!(
            said(&Notice::Lost(vec![(notes, "gone.md".into())])),
            ("notes links to gone, which is gone".into(), None)
        );
        assert_eq!(
            said(&Notice::Lost(vec![(notes, "gone.md".into()), (log, "gone.md".into())])),
            ("2 notes link to gone, which is gone".into(), None)
        );

        let by_id = stray(log, "[[plan]]", plan, Some(Mend::Dest(format!("lb://{plan}"))));
        let apart = stray(plan, "ref.md", notes, None);
        let mut index = LinkIndex::default();
        index.set(&files, plan, 0, crate::links::extract("see [the notes][n]\n\n[n]: ref.md\n"));
        let strays = Notice::Strays(vec![one, by_id, embed, apart]);
        let listed = rows(&strays, &files, &index, &writable);
        let listed: Vec<_> = listed
            .iter()
            .map(|r| (r.file, r.name.as_str(), r.detail.as_str(), r.update))
            .collect();
        assert_eq!(
            listed,
            [
                (notes, "notes", "old.md → plan.md", true),
                (log, "log", "[[plan]] → a link by id", true),
                (log, "log", "chart.png · outside what's shared", false),
                (plan, "plan", "ref.md · update by hand", false),
            ]
        );
    }
}
