use gtk::glib;

impl super::App {
    pub fn open_search(&self) {
        self.titlebar.toggle_search_on();
    }

    pub fn update_search(&self) {
        let input = self.titlebar.search_input();
        let (tx, rx) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);

        // Do the work of getting the search results in a separate thread.
        let api = self.api.clone();
        std::thread::spawn(move || {
            let result = api.search_file_paths(&input);
            tx.send(result).unwrap();
        });

        // Act on the search results.
        let app = self.clone();
        rx.attach(None, move |result| {
            match result {
                Ok(data) => app.process_search_result_data(&data),
                Err(err) => app.show_err_dialog(&format!("{:?}", err)),
            }
            glib::Continue(false)
        });
    }

    fn process_search_result_data(&self, data: &[lb::SearchResultItem]) {
        // Re-populate completion list.
        let model = self.titlebar.search_completion_model();
        model.clear();
        for res in data {
            model.set(&model.append(), &[(0, &res.path), (1, &res.id.to_string())]);
        }
    }
}
