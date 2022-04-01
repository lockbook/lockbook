use gtk::glib;
use gtk::prelude::*;

/*struct Possibility {
    path: String,
    id: lb::Uuid,
    score: u32,
}*/

impl super::App {
    pub fn open_search(&self) {
        let username = self.api.root().unwrap().decrypted_name;

        let files = self.api.list_metadatas().unwrap();

        let sort_model = self.titlebar.search_completion_model();
        let list_model = sort_model.model().downcast::<gtk::ListStore>().unwrap();

        //let mut possibs = Vec::new();
        for f in files {
            let path = self.api.path_by_id(f.id).unwrap();
            let path = path.strip_prefix(&username).unwrap_or(&path).to_string();
            list_model.set(&list_model.append(), &[(0, &path), (1, &f.id.to_string()), (2, &0)]);
            /*possibs.push(Possibility {
                path: path.strip_prefix(&username).unwrap_or(&path).to_string(),
                id: f.id,
                score: 0,
            });*/
        }

        self.titlebar.toggle_search_on();
    }

    pub fn update_search(&self) {
        let input = self.titlebar.search_input();
        if input.is_empty() {
            //self.titlebar.search_completion_model().clear();
            return;
        }

        println!("updating search for input {}", input);
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
        /*let model = self.titlebar.search_completion_model();
        model.clear();
        for res in data {
            model.set(&model.append(), &[(0, &res.path), (1, &res.id.to_string())]);
        }*/
    }

    //fn exec_search_field(&self, use_best_match: bool) {}
}
