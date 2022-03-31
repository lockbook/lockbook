impl super::App {
    pub fn open_search(&self) {
        self.header_bar.click_search_button();
    }

    pub fn update_search(&self) {
        // Get current input.
        let input = self.header_bar.search_input();

        // Get the search results.
        let search_results = match self.api.search_file_paths(&input) {
            Ok(results) => results,
            Err(err) => {
                self.show_err_dialog(&format!("{:?}", err));
                return;
            }
        };

        // Re-populate completion list.
        let model = self.header_bar.search_completion_model();
        model.clear();
        for res in search_results {
            model.set(&model.append(), &[(0, &res.path), (1, &res.id.to_string())]);
        }
    }
}
