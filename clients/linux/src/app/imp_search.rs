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

    fn update_search(&self) {
        let (tx, rx) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);

        // Do the work of getting the search results in a separate thread.
        let input = self.titlebar.search_input();
        let api = self.api.clone();
        std::thread::spawn(move || {
            let result = api.search_file_paths(&input);
            tx.send(result).unwrap();
        });

        // Act on the search results when they arrive.
        let app = self.clone();
        rx.attach(None, move |result| {
            match result {
                Ok(data) => app.repopulate_search_results(&data),
                Err(err) => app.show_err_dialog(&format!("{:?}", err)),
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
            self.titlebar.toggle_search_off();
            self.titlebar.clear_search_box();
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
