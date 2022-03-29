impl super::App {
    pub fn close_file(&self) {
        if let Some(tab) = self.account.current_tab() {
            let id = tab.id();

            if self.bg_state.is_dirty(id) {
                self.save_file(Some(id));
            }

            self.bg_state.untrack(id);
            let t = &self.account.tabs;
            t.remove_page(t.current_page());
        }
    }
}
