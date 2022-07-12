use gtk::glib;
use gtk::prelude::*;

use crate::ui;

impl super::App {
    pub fn perform_sync(&self) {
        if self.sync_lock.try_lock().is_err() {
            return;
        }

        self.account.sync.set_started();

        let (tx, rx) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);

        let core = self.api.clone();
        let sync_lock = self.sync_lock.clone();
        std::thread::spawn(move || {
            let _lock = sync_lock.lock().unwrap();
            let closure = {
                let tx = tx.clone();
                move |msg| tx.send(lb::SyncProgressReport::Update(msg)).unwrap()
            };
            let result = core
                .sync(Some(Box::new(closure)))
                .map_err(lb::SyncError::from);
            tx.send(lb::SyncProgressReport::Done(result)).unwrap();
        });

        let app = self.clone();
        rx.attach(None, move |pr: lb::SyncProgressReport| {
            match pr {
                lb::SyncProgressReport::Update(msg) => app.account.sync.set_progress(&msg),
                lb::SyncProgressReport::Done(result) => match result {
                    Ok(()) => {
                        app.account.sync.set_done(Ok("".to_string()));
                        app.refresh_tree_and_tabs();
                        app.update_sync_status();
                    }
                    Err(err) => match err {
                        lb::SyncError::Minor(msg) => app.account.sync.set_done(Err(msg)),
                        lb::SyncError::Major(msg) => eprintln!("{}", msg), //todo: show dialog or something
                    },
                },
            }
            glib::Continue(true)
        });
    }

    fn refresh_tree_and_tabs(&self) {
        let mut all_files = match self.api.list_metadatas() {
            Ok(metas) => metas,
            Err(err) => {
                self.show_err_dialog(&format!("listing metadatas: {}", err));
                return;
            }
        };

        let tabs = &self.account.tabs;
        'tabloop: for i in (0..tabs.n_pages()).rev() {
            let tab = tabs
                .nth_page(Some(i))
                .unwrap()
                .downcast::<ui::Tab>()
                .unwrap();
            for f in &all_files {
                if f.id == tab.id() {
                    tab.set_name(&f.decrypted_name);
                    if Some(i) == tabs.current_page() {
                        self.window.set_title(Some(&f.decrypted_name));
                    }
                    continue 'tabloop;
                }
            }
            tabs.remove_page(Some(i));
        }

        let tree = &self.account.tree;

        let mut expanded_paths = Vec::<gtk::TreePath>::new();
        tree.model.foreach(|_model, tpath, _iter| -> bool {
            if tree.view.row_expanded(tpath) {
                expanded_paths.push(tpath.clone());
            }
            false
        });

        let sel = tree.view.selection();
        let (selected_paths, _) = sel.selected_rows();

        tree.model.clear();
        tree.populate(&mut all_files);

        for path in expanded_paths {
            tree.view.expand_row(&path, false);
        }
        for path in selected_paths {
            sel.select_path(&path);
        }
    }
}
