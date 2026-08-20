//! Shell tab strip + Workspace content (`show_tabs = false`, desktop tab policy).

use egui::Ui;
use workspace_rs::file_cache::FilesExt;

use crate::components::Theme;

use super::ShellApp;
use super::action::Action;
use super::tabs;

pub fn show(app: &mut ShellApp, ui: &mut Ui, _t: &Theme, queue: &mut Vec<Action>) {
    ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);

    tabs::show(app, ui, _t, queue);

    let sidebar_open = app.sidebar_open;
    let failures = {
        let Some(ready) = app.session.ready_mut() else {
            return;
        };

        ready.workspace.show_tabs = false;
        ready.workspace.desktop_tab_policy = true;
        ready.workspace.sidebar_open = sidebar_open;
        if let Some(id) = ready.cursor {
            let parent = {
                let files = ready.workspace.files.read().unwrap();
                files
                    .get_by_id(id)
                    .map(|f| if f.is_folder() { f.id } else { f.parent })
            };
            ready.workspace.focused_parent = parent;
        }

        let out = ready.workspace.show(ui);

        if out.file_cache_updated {
            super::ops::note_files_changed(ready);
        }

        if let Some(Ok(file)) = out.file_created {
            if file.is_document() {
                ready.select_only(file.id);
                // Workspace already opened the doc; expand + animate in sidebar.
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
