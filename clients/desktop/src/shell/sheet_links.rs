//! What a rename, move, or delete does to links, as its sheet says it: a
//! switch to update the links it would break, and the ones it breaks anyway.

use lb::Uuid;
use workspace_rs::file_cache::{Change, FilesExt as _};
use workspace_rs::link_plan::Plan;

use crate::components::compounds::form::{metrics, toggle};
use crate::components::{Space, Spacer, Theme, TypeRole, display_file_name};

use super::ShellApp;
use super::action::Action;

/// Under a sheet's summary: nothing when `change` touches no links.
pub(crate) fn show_link_plan(
    app: &ShellApp, ui: &mut egui::Ui, t: &Theme, queue: &mut Vec<Action>, change: &Change,
) {
    let Some(r) = app.session.ready() else { return };
    let plan = r.workspace.link_plan(change);
    if plan.is_empty() {
        return;
    }
    let names = |notes: Vec<Uuid>| -> Vec<String> {
        let files = r.workspace.files.read().unwrap();
        notes
            .into_iter()
            .filter_map(|id| files.get_by_id(id))
            .map(|f| display_file_name(&f.name).to_owned())
            .collect()
    };
    if !plan.mends.is_empty() {
        ui.add(Spacer::new(Space::Md));
        let mut on = r.workspace.cfg.get_update_links();
        let spans = update_spans(&plan, &names(plan.mended_notes()));
        if switch_row(ui, t, &spans, &mut on) {
            queue.push(Action::SetPrefUpdateLinks(on));
        }
    }
    if !plan.stuck.is_empty() {
        ui.add(Spacer::new(Space::Md));
        paragraph(ui, &break_spans(&plan, &names(plan.stuck_notes())), t.neutral_fg_secondary());
    }
    if plan.checking {
        checking(ui, t);
    }
}

/// While some notes haven't been read yet, what's listed may be short.
fn checking(ui: &mut egui::Ui, t: &Theme) {
    let line = [("Still reading your notes, so some links may be missing here.".into(), false)];
    ui.add(Spacer::new(Space::Md));
    paragraph(ui, &line, t.neutral_fg_secondary());
}

/// Update **3 links** in **log**, **roadmap**, and **2 other notes**.
fn update_spans(plan: &Plan, notes: &[String]) -> Vec<(String, bool)> {
    let mut spans = vec![("Update ".into(), false), (count(plan.mends.len(), "link"), true)];
    spans.push((" in ".into(), false));
    spans.extend(note_list(notes));
    spans.push((".".into(), false));
    spans
}

/// **2 links** in **theirs** will break.
fn break_spans(plan: &Plan, notes: &[String]) -> Vec<(String, bool)> {
    let mut spans = vec![(count(plan.stuck.len(), "link"), true), (" in ".into(), false)];
    spans.extend(note_list(notes));
    spans.push((" will break.".into(), false));
    spans
}

fn count(n: usize, noun: &str) -> String {
    if n == 1 { format!("1 {noun}") } else { format!("{n} {noun}s") }
}

/// Two names, then how many others: **a**, **b**, and **3 other notes**.
fn note_list(names: &[String]) -> Vec<(String, bool)> {
    name_list(names, "other note", "other notes")
}

fn name_list(names: &[String], other: &str, others_: &str) -> Vec<(String, bool)> {
    let shown = if names.len() <= 3 { names.len() } else { 2 };
    let mut items: Vec<String> = names[..shown].to_vec();
    let others = names.len() - shown;
    if others == 1 {
        items.push(format!("1 {other}"));
    } else if others > 1 {
        items.push(format!("{others} {others_}"));
    }
    let mut spans = vec![];
    for (i, item) in items.iter().enumerate() {
        if i > 0 {
            let sep = match (items.len(), i + 1 == items.len()) {
                (2, _) => " and ",
                (_, true) => ", and ",
                _ => ", ",
            };
            spans.push((sep.into(), false));
        }
        spans.push((item.clone(), true));
    }
    spans
}

/// A paragraph with a switch at its right, on its first line.
fn switch_row(ui: &mut egui::Ui, t: &Theme, spans: &[(String, bool)], on: &mut bool) -> bool {
    let width = crate::components::ui_width(ui);
    let gap = Space::Md.pts();
    let top = ui.cursor().min;
    let text_w = width - metrics::TOGGLE_W - gap;
    let text = egui::Rect::from_min_size(top, egui::vec2(text_w, f32::INFINITY));
    let mut text_ui = ui.new_child(egui::UiBuilder::new().max_rect(text));
    paragraph(&mut text_ui, spans, t.neutral_fg());
    // the switch centers on the first line
    let line = TypeRole::Body.line_height();
    let at =
        egui::pos2(top.x + width - metrics::TOGGLE_W, top.y + (line - metrics::TOGGLE_H) / 2.0);
    let switch = egui::Rect::from_min_size(at, egui::vec2(metrics::TOGGLE_W, metrics::TOGGLE_H));
    let changed =
        toggle(&mut ui.new_child(egui::UiBuilder::new().max_rect(switch)), t, on).changed();
    let height = text_ui.min_rect().height().max(metrics::TOGGLE_H);
    ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::hover());
    changed
}

fn paragraph(ui: &mut egui::Ui, spans: &[(String, bool)], ink: egui::Color32) {
    use workspace_rs::widgets::GlyphonLabel;
    let rich: Vec<(&str, bool)> = spans.iter().map(|(s, b)| (s.as_str(), *b)).collect();
    let max_w = crate::components::ui_width(ui).max(1.0);
    ui.add(
        GlyphonLabel::new_rich(rich, ink)
            .font_size(TypeRole::Body.size())
            .line_height(TypeRole::Body.line_height())
            .max_width(max_w),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(spans: Vec<(String, bool)>) -> String {
        spans
            .into_iter()
            .map(|(s, b)| if b { format!("**{s}**") } else { s })
            .collect()
    }

    #[test]
    fn notes_are_named_then_counted() {
        let n = |names: &[&str]| {
            text(note_list(&names.iter().map(|s| s.to_string()).collect::<Vec<_>>()))
        };
        assert_eq!(n(&["a"]), "**a**");
        assert_eq!(n(&["a", "b"]), "**a** and **b**");
        assert_eq!(n(&["a", "b", "c"]), "**a**, **b**, and **c**");
        assert_eq!(n(&["a", "b", "c", "d"]), "**a**, **b**, and **2 other notes**");
    }
}
