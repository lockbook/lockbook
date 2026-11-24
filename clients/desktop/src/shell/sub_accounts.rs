use std::sync::mpsc::{self, Receiver, Sender};

use lb::model::account::SUB_ACCOUNT_SEPARATOR;
use lb::model::api::SubAccountInfo;

pub const KEY_ICON: &str = "\u{e2d6}";

pub const ABOUT: &str = "A sub-account has its own keys for a script, a tool or another person. It uses your storage and reaches only what you share with it.";

const MAX_SUGGESTIONS: usize = 5;

pub struct SubAccountKeys {
    pub username: String,
    pub key: String,
    pub phrase: String,
}

pub(crate) enum Done {
    Listed(Result<Vec<SubAccountInfo>, String>),
    Created(Result<String, String>),
    Deleted { username: String, result: Result<(), String> },
    Keys { username: String, result: Result<(String, String), String> },
}

pub struct SubAccounts {
    pub list: Vec<SubAccountInfo>,
    pub loaded: bool,
    pub load_error: Option<String>,
    pub working: bool,
    pub error: Option<String>,
    pub keys: Option<SubAccountKeys>,
    pub(crate) tx: Sender<Done>,
    pub(crate) rx: Receiver<Done>,
}

impl Default for SubAccounts {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            list: Vec::new(),
            loaded: false,
            load_error: None,
            working: false,
            error: None,
            keys: None,
            tx,
            rx,
        }
    }
}

impl SubAccounts {
    pub fn usernames(&self) -> Vec<String> {
        self.list.iter().map(|s| s.username.clone()).collect()
    }

    pub fn contains(&self, username: &str) -> bool {
        self.list
            .iter()
            .any(|s| s.username.eq_ignore_ascii_case(username.trim()))
    }

    pub fn usage(&self, username: &str) -> Option<u64> {
        self.list
            .iter()
            .find(|s| s.username.eq_ignore_ascii_case(username))
            .map(|s| s.usage)
    }
}

pub fn is_sub_account(username: &str) -> bool {
    username.contains(SUB_ACCOUNT_SEPARATOR)
}

pub fn name_example(parent: &str) -> String {
    format!("{parent}{SUB_ACCOUNT_SEPARATOR}label")
}

pub fn share_suggestions(
    query: &str, sub_accounts: &[String], me: &str, staged: &[String], shared: &[String],
) -> Vec<String> {
    let prefix = query.trim().to_lowercase();
    let own = format!("{}{SUB_ACCOUNT_SEPARATOR}", me.to_lowercase());
    let has = |names: &[String], lower: &str| names.iter().any(|n| n.to_lowercase() == lower);

    let mut out: Vec<String> = sub_accounts
        .iter()
        .filter(|name| {
            let lower = name.to_lowercase();
            if lower == prefix || has(staged, &lower) {
                return false;
            }
            if prefix.is_empty() {
                return !has(shared, &lower);
            }
            let label = lower.strip_prefix(&own).unwrap_or(&lower);
            lower.starts_with(&prefix)
                || label.starts_with(&prefix)
                || format!("{SUB_ACCOUNT_SEPARATOR}{label}").starts_with(&prefix)
        })
        .cloned()
        .collect();
    out.sort_by_key(|name| (name.chars().count(), name.to_lowercase()));
    out.truncate(MAX_SUGGESTIONS);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn sub_account_names_have_a_separator() {
        assert!(is_sub_account("jane+site"));
        assert!(!is_sub_account("jane"));
        assert_eq!(name_example("jane"), "jane+label");
    }

    #[test]
    fn empty_query_lists_what_is_not_shared_or_staged() {
        let subs = names(&["jane+site", "jane+agent", "jane+ci"]);
        let got =
            share_suggestions("", &subs, "jane", &names(&["jane+ci"]), &names(&["Jane+Site"]));
        assert_eq!(got, names(&["jane+agent"]));
    }

    #[test]
    fn query_matches_full_name_label_or_plus_label() {
        let subs = names(&["jane+site", "jane+agent"]);
        for query in ["jane+s", "si", "+si", "SI"] {
            assert_eq!(
                share_suggestions(query, &subs, "jane", &[], &[]),
                names(&["jane+site"]),
                "{query}"
            );
        }
        assert_eq!(share_suggestions("j", &subs, "jane", &[], &[]).len(), 2);
        assert!(share_suggestions("bob", &subs, "jane", &[], &[]).is_empty());
    }

    #[test]
    fn typing_still_offers_one_already_shared() {
        let subs = names(&["jane+site"]);
        let shared = names(&["jane+site"]);
        assert_eq!(share_suggestions("si", &subs, "jane", &[], &shared), subs);
    }

    #[test]
    fn exact_name_and_staged_are_left_out() {
        let subs = names(&["jane+site", "jane+sitemap"]);
        assert_eq!(
            share_suggestions("jane+site", &subs, "jane", &[], &[]),
            names(&["jane+sitemap"])
        );
        assert!(
            share_suggestions("si", &subs, "jane", &names(&["jane+site", "JANE+SITEMAP"]), &[])
                .is_empty()
        );
    }

    #[test]
    fn at_most_five_shortest_first() {
        let subs = names(&[
            "jane+ffffff",
            "jane+eeeee",
            "jane+dddd",
            "jane+ccc",
            "jane+bb",
            "jane+a",
            "jane+b",
        ]);
        assert_eq!(
            share_suggestions("", &subs, "jane", &[], &[]),
            names(&["jane+a", "jane+b", "jane+bb", "jane+ccc", "jane+dddd"])
        );
    }
}
