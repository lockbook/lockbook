use gtk::glib;
use gtk::prelude::*;

use crate::ui;

impl super::App {
    pub fn listen_for_search_ops(&self) {
        let app = self.clone();
        self.titlebar.receive_search_ops(move |op| {
            match op {
                ui::SearchOp::Exec => app.exec_search(),
            }
            glib::Continue(true)
        });
    }

    pub fn open_search(&self) {
        self.titlebar.toggle_search_on();

        let (tx, rx) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);

        let api = self.api.clone();
        std::thread::spawn(move || {
            let result = api.searcher(Some(lb::Filter::DocumentsOnly));
            tx.send(result).unwrap();
        });

        let app = self.clone();
        rx.attach(None, move |searcher_result| {
            match searcher_result {
                Ok(searcher) => app.titlebar.set_searcher(searcher),
                Err(msg) => app.show_err_dialog(&msg),
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
            self.repopulate_search_results(&[]);
            self.open_file(id);
        } else {
            self.show_err_dialog("no file path matches your search!");
        }
    }

    fn repopulate_search_results(&self, data: &[lb::SearchResultItem]) {
        let list = self.titlebar.search_result_list();

        while let Some(row) = list.row_at_index(0) {
            list.remove(&row);
        }

        for res in data {
            let row = ui::SearchRow::new();
            row.set_data(res.id, &res.path);
            list.append(&row);
        }
    }
}
