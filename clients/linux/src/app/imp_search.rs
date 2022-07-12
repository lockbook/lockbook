use gtk::glib;
use gtk::prelude::*;

use crate::ui;

impl super::App {
    pub fn listen_for_search_ops(&self) {
        let app = self.clone();
        self.titlebar.receive_search_ops(move |op| {
            match op {
                ui::SearchOp::Update => app.update_search(),
                ui::SearchOp::Exec => app.exec_search(),
            }
            glib::Continue(true)
        });
    }

    pub fn open_search(&self) {
        self.titlebar.toggle_search_on();
    }

    pub fn update_search(&self) {
        self.titlebar.waiting_for_search_results();

        let (tx, rx) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);

        let core = self.core.clone();
        let input = self.titlebar.search_input();
        std::thread::spawn(move || {
            let result = core.search_file_paths(&input);
            tx.send(result).unwrap();
        });

        let app = self.clone();
        rx.attach(None, move |search_results| {
            match search_results {
                Ok(results) => app.titlebar.show_search_results(&results),
                Err(msg) => app.show_err_dialog(&msg.0),
            }
            glib::Continue(false)
        });
    }

    fn exec_search(&self) {
        let list = self.titlebar.search_result_list();
        if let Some(row_choice) = list.selected_row().or_else(|| list.row_at_index(0)) {
            let id = row_choice
                .child()
                .unwrap()
                .downcast_ref::<ui::SearchRow>()
                .unwrap()
                .id();
            self.titlebar.clear_search();
            self.open_file(id);
        } else {
            self.show_err_dialog("no file path matches your search!");
        }
    }
}
