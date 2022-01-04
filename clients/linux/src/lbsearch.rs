use std::cmp;

use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};
use gtk::prelude::*;

use crate::app::LbApp;
use crate::error::LbResult;
use crate::messages::Msg;
use crate::tree_iter_value;
use crate::util;

pub struct LbSearch {
    possibs: Vec<String>,
    list_store: gtk::ListStore,
    pub sort_model: gtk::TreeModelSort,
    matcher: SkimMatcherV2,
}

impl LbSearch {
    pub fn new(possibs: Vec<String>) -> Self {
        let list_store = gtk::ListStore::new(&[i64::static_type(), String::static_type()]);
        let sort_model = gtk::TreeModelSort::new(&list_store);
        sort_model.set_sort_column_id(gtk::SortColumn::Index(0), gtk::SortType::Descending);
        sort_model.set_sort_func(gtk::SortColumn::Index(0), Self::cmp_possibs);

        Self {
            possibs,
            list_store,
            sort_model,
            matcher: SkimMatcherV2::default(),
        }
    }

    fn cmp_possibs(
        model: &gtk::TreeModel,
        it1: &gtk::TreeIter,
        it2: &gtk::TreeIter,
    ) -> cmp::Ordering {
        let score1 = tree_iter_value!(model, it1, 0, i64);
        let score2 = tree_iter_value!(model, it2, 0, i64);

        match score1.cmp(&score2) {
            cmp::Ordering::Greater => cmp::Ordering::Greater,
            cmp::Ordering::Less => cmp::Ordering::Less,
            cmp::Ordering::Equal => {
                let text1 = tree_iter_value!(model, it1, 1, String);
                let text2 = model
                    .get_value(it2, 1)
                    .get::<String>()
                    .unwrap_or_default()
                    .unwrap_or_default();
                if text2.is_empty() {
                    return cmp::Ordering::Less;
                }

                let chars1: Vec<char> = text1.chars().collect();
                let chars2: Vec<char> = text2.chars().collect();

                let n_chars1 = chars1.len();
                let n_chars2 = chars2.len();

                for i in 0..cmp::min(n_chars1, n_chars2) {
                    let ord = chars1[i].cmp(&chars2[i]);
                    if ord != cmp::Ordering::Equal {
                        return ord.reverse();
                    }
                }

                n_chars1.cmp(&n_chars2)
            }
        }
    }

    pub fn update_for(&self, pattern: &str) {
        let list = &self.list_store;
        list.clear();

        for p in &self.possibs {
            if let Some(score) = self.matcher.fuzzy_match(p, pattern) {
                let values: [&dyn ToValue; 2] = [&score, &p];
                list.set(&list.append(), &[0, 1], &values);
            }
        }
    }
}

pub fn prompt_search(lb: &LbApp) -> LbResult<()> {
    let possibs = lb.core.list_paths().unwrap_or_default();
    let search = LbSearch::new(possibs);
    let d = lb.gui.new_dialog("Search");

    let comp = gtk::EntryCompletion::new();
    comp.set_model(Some(&search.sort_model));
    comp.set_popup_completion(true);
    comp.set_inline_selection(true);
    comp.set_text_column(1);
    comp.set_match_func(|_, _, _| true);
    comp.connect_match_selected(glib::clone!(
        @strong lb.messenger as m,
        @strong d
        => move |_, model, iter| {
            d.close();
            let iter_val = tree_iter_value!(model, iter, 1, String);
            m.send(Msg::SearchFieldExec(Some(iter_val)));
            gtk::Inhibit(false)
        }
    ));

    lb.state.borrow_mut().search = Some(search);

    let search_entry = gtk::Entry::new();
    util::gui::set_entry_icon(&search_entry, "edit-find-symbolic");
    util::gui::set_marginx(&search_entry, 16);
    search_entry.set_margin_top(16);
    search_entry.set_placeholder_text(Some("Start typing..."));
    search_entry.set_completion(Some(&comp));

    search_entry.connect_key_release_event(glib::clone!(@strong lb => move |entry, key| {
        let k = key.get_hardware_keycode();
        if k != util::gui::KEY_ARROW_UP && k != util::gui::KEY_ARROW_DOWN {
            if let Some(search) = lb.state.borrow().search.as_ref() {
                let input = entry.get_text().to_string();
                search.update_for(&input);
            }
        }
        gtk::Inhibit(false)
    }));

    search_entry.connect_changed(move |entry| {
        let input = entry.get_text().to_string();
        let icon_name = if input.ends_with(".md") || input.ends_with(".txt") {
            "text-x-generic-symbolic"
        } else if input.ends_with('/') {
            "folder-symbolic"
        } else {
            "edit-find-symbolic"
        };
        util::gui::set_entry_icon(entry, icon_name);
    });

    search_entry.connect_activate(
        glib::clone!(@strong lb.messenger as m, @strong d => move |entry| {
            d.close();
            if !entry.get_text().eq("") {
                m.send(Msg::SearchFieldExec(None));
            }
        }),
    );

    d.set_size_request(400, -1);
    d.get_content_area().add(&search_entry);
    d.get_content_area().set_margin_bottom(16);
    d.show_all();
    Ok(())
}

pub fn search_field_exec(lb: &LbApp, maybe_input: Option<String>) -> LbResult<()> {
    if let Some(path) = maybe_input.or_else(|| lb.state.borrow().get_first_search_match()) {
        match lb.core.file_by_path(&path) {
            Ok(meta) => lb.messenger.send(Msg::OpenFile(Some(meta.id))),
            Err(err) => lb
                .messenger
                .send_err_dialog("opening file from search field", err),
        }
    }
    Ok(())
}
