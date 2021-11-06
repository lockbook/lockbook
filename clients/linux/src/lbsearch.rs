use std::cmp;

use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};
use gtk::prelude::*;

use crate::tree_iter_value;

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
                    .get_value(&it2, 1)
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
            if let Some(score) = self.matcher.fuzzy_match(&p, &pattern) {
                let values: [&dyn ToValue; 2] = [&score, &p];
                list.set(&list.append(), &[0, 1], &values);
                //list.set(&list.append(), &[(0, &score), (1, &p)]);
            }
        }
    }
}
