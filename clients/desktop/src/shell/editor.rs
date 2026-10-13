//! Shell tab strip + Workspace content (`show_tabs = false`, desktop tab policy).

use egui::{Align, Layout, Ui};

use crate::components::{Theme, claim, place_at};

use super::ShellApp;
use super::action::Action;
use super::tabs;

pub fn show(app: &mut ShellApp, ui: &mut Ui, _t: &Theme, queue: &mut Vec<Action>) {
    ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);

    tabs::show(app, ui, _t, queue);

    // Workspace (and the empty landing page) must not paint `max_rect` under
    // the titleband — that fill would cover traffic lights / pane cluster.
    let rest = ui.available_rect_before_wrap();
    let sidebar_open = app.sidebar_open;
    let failures = {
        let Some(ready) = app.session.ready_mut() else {
            return;
        };

        ready.workspace.show_tabs = false;
        ready.workspace.desktop_tab_policy = true;
        ready.workspace.sidebar_open = sidebar_open;
        // Workspace create-dest follows the open tab. Tree `cursor` is selection
        // only — a folder right-click must not retarget ⌘N / landing create.
        ready.workspace.focused_parent = ready.workspace.current_tab_id();

        let (out, _) =
            place_at(ui, rest, Layout::top_down(Align::Min), |ui| ready.workspace.show(ui));
        claim(ui, rest);

        if out.file_cache_updated {
            super::ops::note_files_changed(ready);
        }

        if let Some(Ok(file)) = out.file_created {
            if file.is_document() {
                ready.select_only(file.id);
                // Workspace already opened the doc; expand + reveal in the Files tree.
                super::reveal_and_scroll(ready, file.id);
            }
        }

        // Tab focus / persistence restore: workspace says which file is current;
        if let Some(id) = out.selected_file {
            ready.select_only(id);
            super::reveal_and_scroll(ready, id);
        }

        out.failure_messages
    };
    for msg in failures {
        app.toasts.error(msg);
    }
}
