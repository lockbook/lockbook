use gtk::glib;
use gtk::prelude::*;

use crate::ui;

impl super::App {
    pub fn tree_receive_drop(&self, val: &glib::Value, x: f64, y: f64) {
        let drop_str = val.get::<String>().unwrap();
        let uris = drop_str
            .split("\r\n")
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect::<Vec<String>>();

        let ftree = &self.account.tree;
        let x = x as i32;
        let y = y as i32;

        let dest = match ftree.view.path_at_pos(x, y) {
            Some((Some(tpath), _, _, _)) => ui::id_from_tpath(&ftree.model, &tpath),
            _ => match self.api.get_root() {
                Ok(fm) => fm.id,
                Err(err) => {
                    self.show_err_dialog(&format!("{:?}", err));
                    return;
                }
            },
        };

        self.import_files(uris, dest);
    }
}
