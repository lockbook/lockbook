use gtk::prelude::*;

use crate::ui;

impl super::App {
    pub fn tree_toggle_col(&self, col: ui::FileTreeCol) {
        if col != ui::FileTreeCol::IconAndName {
            self.toggle_in_tree_view(col);
            self.toggle_in_settings(col);
        }
    }

    fn toggle_in_tree_view(&self, col: ui::FileTreeCol) {
        let t = &self.account.tree;
        let col_name = col.name();

        // Look for the column first and remove it if found.
        for c in &t.view.columns() {
            if c.title().eq(&col_name) {
                t.view.remove_column(c);
                return;
            }
        }

        // Column wasn't found and removed, so it gets added at the proper index.
        let mut i = col.as_tree_store_index();
        while i > 0 {
            i -= 1;
            if let Some(prev) = t.cols.get(i as usize) {
                if tree_view_has_col(&t.view, prev) {
                    t.view.insert_column(&col.as_tree_view_col(), i + 1);
                    return;
                }
            }
        }
    }

    fn toggle_in_settings(&self, col: ui::FileTreeCol) {
        let col_name = col.name();
        let mut settings = self.settings.write().unwrap();
        let cols = &mut settings.hidden_tree_cols;
        match cols.contains(&col_name) {
            true => cols.retain(|c| !c.eq(&col_name)),
            false => cols.push(col_name),
        }
    }
}

fn tree_view_has_col(tv: &gtk::TreeView, col: &ui::FileTreeCol) -> bool {
    for c in tv.columns() {
        if c.title().eq(&col.name()) {
            return true;
        }
    }
    false
}
