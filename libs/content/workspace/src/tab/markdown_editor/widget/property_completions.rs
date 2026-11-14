//! Completions in the front matter grid: property names while a key is
//! typed, and the values other notes gave that key while a value or list
//! item is typed. Candidates come from the note index; the nearest note
//! that uses one ranks it, so a folder's own vocabulary comes first.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use egui::{Context, Key, Modifiers, Ui};
use lb_rs::Uuid;
use lb_rs::model::text::buffer::Buffer;
use lb_rs::model::text::offset_types::{Grapheme, RangeExt as _};

use crate::file_cache::{FileCache, FilesExt as _};
use crate::front_matter::{FrontMatter, RowKind as Kind, Value};
use crate::tab::markdown_editor::MdEdit;
use crate::tab::markdown_editor::input::{Event, Location, Region};
use crate::tab::markdown_editor::widget::{
    CompletionStep, completion_accepted, completion_dismissed, completion_step,
};

const MAX_RESULTS: usize = 5;

/// Keys every vault starts with, offered when no note has them yet.
const DEFAULT_KEYS: [&str; 2] = ["tags", "aliases"];

/// Where candidates come from.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Pool {
    Keys,
    Values(String),
}

/// What the caret is typing, and what accepting a candidate replaces.
#[derive(Clone, Debug)]
struct Target {
    replace: (Grapheme, Grapheme),
    query: String,
    pool: Pool,
    /// A key being typed: accepting it writes the colon too.
    commit_key: bool,
    /// A value after a bare `key:` needs a space before it.
    space: bool,
    /// Already in the block or the list; not offered again.
    exclude: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Candidate {
    pub text: String,
    /// Notes with it.
    pub count: usize,
    /// Folders to the nearest note with it.
    pub distance: usize,
}

#[derive(Default)]
pub struct PropertyCompletions {
    /// True when a key, value, or item is being typed and has candidates.
    /// Read by the editor to gate rendering; also gates `handle_input`.
    pub active: bool,
    /// Keyboard-highlighted result index.
    pub selected: usize,
    /// The typed text, drawn in the accent.
    pub search_term_range: Option<(Grapheme, Grapheme)>,
    /// A query dismissed with Escape stays dismissed until it changes.
    suppressed: Option<String>,
    target: Option<Target>,
    /// The ranked candidates for the target, top first.
    pub results: Vec<Candidate>,
    /// The pool's candidates, by index version, note, and pool.
    pool: Option<(u64, Uuid, Pool, Vec<Candidate>)>,
}

impl PropertyCompletions {
    /// Recomputes whether the popup should be active. Must be called before
    /// `handle_input`, which early-returns when `self.active` is false.
    pub fn update_active_state(
        &mut self, buffer: &Buffer, fm: Option<&FrontMatter<Grapheme>>,
        files: &Arc<RwLock<FileCache>>, file_id: Uuid,
    ) {
        self.active = false;
        self.search_term_range = None;
        self.target = None;
        self.results.clear();
        let Some(target) = fm.and_then(|fm| target(buffer, fm)) else { return };
        if self.suppressed.as_deref() == Some(target.query.as_str()) {
            return;
        }
        let results = self.search(files, file_id, &target);
        if results.is_empty() {
            return;
        }
        self.active = true;
        if !target.query.is_empty() {
            self.search_term_range =
                Some((target.replace.start(), target.replace.start() + target.query_len()));
        }
        self.selected = self.selected.min(results.len() - 1);
        self.results = results;
        self.target = Some(target);
    }

    /// The pool's candidates ranked for the query: how the query matches,
    /// then folders to the nearest note that has it, then how many do,
    /// then the text — an order the query takes no part in within a class,
    /// so the top candidate holds as the rest of it is typed.
    fn search(
        &mut self, files: &Arc<RwLock<FileCache>>, file_id: Uuid, target: &Target,
    ) -> Vec<Candidate> {
        let cache = files.read().unwrap();
        cache.notes.want();
        let version = cache.notes.version();
        let stale = !matches!(&self.pool, Some((v, id, pool, _)) if *v == version && *id == file_id && *pool == target.pool);
        if stale {
            let candidates = pool_candidates(&cache, file_id, &target.pool);
            self.pool = Some((version, file_id, target.pool.clone(), candidates));
        }
        let candidates = &self.pool.as_ref().unwrap().3;
        rank(candidates, &target.query, &target.exclude)
    }

    /// Consume (or observe) keyboard events targeting the popup. Emitted
    /// replacements are pushed onto `events`. Escape is observed, not
    /// consumed, and works unfocused; the rest only with focus.
    pub fn handle_input(&mut self, ctx: &Context, editor_focused: bool, events: &mut Vec<Event>) {
        if !self.active || self.results.is_empty() {
            return;
        }
        let Some(target) = self.target.clone() else { return };
        if completion_dismissed(ctx) {
            self.suppressed = Some(target.query);
            self.active = false;
            return;
        }
        if !editor_focused {
            return;
        }
        match completion_step(ctx) {
            Some(CompletionStep::Up) => self.selected = self.selected.saturating_sub(1),
            Some(CompletionStep::Down) => {
                self.selected = (self.selected + 1).min(self.results.len() - 1)
            }
            None => {}
        }
        if completion_accepted(ctx) {
            let text = self.results[self.selected].text.clone();
            self.apply_completion(events, &target, &text);
            return;
        }
        let num_modifier = if cfg!(any(target_os = "macos", target_os = "ios")) {
            Modifiers::COMMAND
        } else {
            Modifiers::CTRL
        };
        for (idx, key) in [Key::Num1, Key::Num2, Key::Num3, Key::Num4, Key::Num5]
            .iter()
            .enumerate()
            .take(self.results.len())
        {
            if ctx.input_mut(|i| i.consume_key(num_modifier, *key)) {
                let text = self.results[idx].text.clone();
                self.apply_completion(events, &target, &text);
                return;
            }
        }
    }

    /// Replace the typed text with the candidate and reset popup state.
    fn apply_completion(&mut self, events: &mut Vec<Event>, target: &Target, text: &str) {
        let text = match (target.commit_key, target.space) {
            (true, _) => format!("{text}: "),
            (false, true) => format!(" {text}"),
            (false, false) => text.to_string(),
        };
        events.push(Event::Replace {
            region: Region::BetweenLocations {
                start: Location::Grapheme(target.replace.start()),
                end: Location::Grapheme(target.replace.end()),
            },
            text,
            advance_cursor: true,
        });
        self.selected = 0;
        self.suppressed = None;
        self.active = false;
    }
}

impl Target {
    fn query_len(&self) -> usize {
        use unicode_segmentation::UnicodeSegmentation as _;
        self.query.graphemes(true).count()
    }
}

/// The key, value, or item the caret is at the end of, if any.
fn target(buffer: &Buffer, fm: &FrontMatter<Grapheme>) -> Option<Target> {
    let selection = buffer.current.selection;
    if !selection.is_empty() {
        return None;
    }
    let offset = selection.0;
    let i = fm.row_at(offset)?;
    let keys: Vec<String> = fm
        .rows
        .iter()
        .filter_map(|r| match &r.kind {
            Kind::Property { key, .. } | Kind::Pending { key } => Some(buffer[*key].to_string()),
            _ => None,
        })
        .collect();
    let others = |own: &str| -> Vec<String> {
        let mut seen = false;
        keys.iter()
            .filter(|k| {
                let same = !seen && k.as_str() == own;
                seen |= same;
                !same
            })
            .cloned()
            .collect()
    };
    match &fm.rows[i].kind {
        Kind::Pending { key } if offset == key.end() && !key.is_empty() => Some(Target {
            replace: *key,
            query: buffer[*key].to_string(),
            pool: Pool::Keys,
            commit_key: true,
            space: false,
            exclude: others(&buffer[*key]),
        }),
        Kind::Property { key, value, .. } => {
            if offset == key.end() && !key.is_empty() {
                return Some(Target {
                    replace: *key,
                    query: buffer[*key].to_string(),
                    pool: Pool::Keys,
                    commit_key: false,
                    space: false,
                    exclude: others(&buffer[*key]),
                });
            }
            let pool = Pool::Values(buffer[*key].to_string());
            match value {
                Value::Text { text, checked: None, .. } if offset == text.end() => Some(Target {
                    replace: *text,
                    query: buffer[*text].to_string(),
                    pool,
                    commit_key: false,
                    space: false,
                    exclude: vec![],
                }),
                Value::Empty { at, needs_space } if offset == *at => Some(Target {
                    replace: (*at, *at),
                    query: String::new(),
                    pool,
                    commit_key: false,
                    space: *needs_space,
                    exclude: vec![],
                }),
                Value::List { items, .. } => {
                    let j = value.item_at(offset)?;
                    let item = &items[j];
                    if offset != item.text.end() {
                        return None;
                    }
                    let exclude = items
                        .iter()
                        .enumerate()
                        .filter(|(k, _)| *k != j)
                        .map(|(_, it)| buffer[it.text].to_string())
                        .collect();
                    Some(Target {
                        replace: item.text,
                        query: buffer[item.text].to_string(),
                        pool,
                        commit_key: false,
                        space: false,
                        exclude,
                    })
                }
                _ => None,
            }
        }
        _ => None,
    }
}

/// Every distinct key, or every distinct value of `key`, across the index,
/// with how many notes have it and the folders to the nearest.
fn pool_candidates(cache: &FileCache, file_id: Uuid, pool: &Pool) -> Vec<Candidate> {
    let mut found: HashMap<String, Candidate> = HashMap::new();
    for (id, note) in cache.notes.notes() {
        // this note's own index is what is being typed
        if id == file_id {
            continue;
        }
        let texts: Vec<&str> = match pool {
            Pool::Keys => note.properties.iter().map(|(k, _)| k.as_str()).collect(),
            Pool::Values(key) => note
                .properties
                .iter()
                .filter(|(k, _)| k == key)
                .flat_map(|(_, v)| v.iter().map(String::as_str))
                .collect(),
        };
        if texts.is_empty() {
            continue;
        }
        let distance = cache.folder_distance(file_id, id);
        let mut counted = std::collections::HashSet::new();
        for text in texts {
            if text.is_empty() {
                continue;
            }
            let lower = text.to_lowercase();
            let c = found.entry(lower.clone()).or_insert_with(|| Candidate {
                text: text.to_string(),
                count: 0,
                distance,
            });
            c.distance = c.distance.min(distance);
            if counted.insert(lower) {
                c.count += 1;
            }
        }
    }
    if *pool == Pool::Keys {
        for key in DEFAULT_KEYS {
            found.entry(key.to_string()).or_insert(Candidate {
                text: key.to_string(),
                count: 0,
                distance: usize::MAX,
            });
        }
    }
    found.into_values().collect()
}

/// How a candidate matches: it starts with the query, a word in it does,
/// or it merely contains it.
fn match_class(text: &str, query: &str) -> Option<u8> {
    if query.is_empty() {
        return Some(0);
    }
    let (t, q) = (text.to_lowercase(), query.to_lowercase());
    if t.starts_with(&q) {
        return Some(0);
    }
    if t.split(['-', '_', ' ', '/', '.'])
        .skip(1)
        .any(|w| w.starts_with(&q))
    {
        return Some(1);
    }
    t.contains(&q).then_some(2)
}

fn rank(candidates: &[Candidate], query: &str, exclude: &[String]) -> Vec<Candidate> {
    let q = query.to_lowercase();
    let excluded = |t: &str| exclude.iter().any(|e| e.eq_ignore_ascii_case(t));
    let mut ranked: Vec<(u8, &Candidate)> = candidates
        .iter()
        .filter(|c| c.text.to_lowercase() != q && !excluded(&c.text))
        .filter_map(|c| Some((match_class(&c.text, query)?, c)))
        .collect();
    ranked.sort_by(|(ca, a), (cb, b)| {
        (ca, a.distance, std::cmp::Reverse(a.count), a.text.to_lowercase()).cmp(&(
            cb,
            b.distance,
            std::cmp::Reverse(b.count),
            b.text.to_lowercase(),
        ))
    });
    ranked
        .into_iter()
        .take(MAX_RESULTS)
        .map(|(_, c)| c.clone())
        .collect()
}

impl MdEdit {
    pub fn show_property_completions(&mut self, ui: &mut Ui) {
        if self.renderer.readonly || !self.property_completions.active {
            return;
        }
        let Some(target) = self.property_completions.target.clone() else { return };
        let results = self.property_completions.results.clone();
        if results.is_empty() {
            return;
        }
        let labels: Vec<String> = results.iter().map(|c| c.text.clone()).collect();
        let highlighted = Some(self.property_completions.selected);
        // anchored at the typed text's start, so it stays while typing
        let clicked = self.show_completion_rows(
            ui,
            target.replace.start(),
            &labels,
            highlighted,
            "property_item",
        );
        if let Some(i) = clicked {
            let text = results[i].text.clone();
            self.property_completions.apply_completion(
                &mut self.event.internal_events,
                &target,
                &text,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(text: &str, distance: usize, count: usize) -> Candidate {
        Candidate { text: text.into(), count, distance }
    }

    fn texts(v: &[Candidate]) -> Vec<&str> {
        v.iter().map(|c| c.text.as_str()).collect()
    }

    /// Rule: candidates rank by how the query matches, the start over a
    /// word start over a mere match, then nearest note, count, and name;
    /// what is typed and what is present are not offered.
    #[test]
    fn candidates_rank_by_match_then_nearness() {
        let near =
            [c("in-progress", 2, 40), c("done", 0, 3), c("in-review", 1, 10), c("idle", 0, 3)];
        let word = [c("progress", 3, 1), c("in-progress", 0, 9), c("reprogram", 0, 9)];
        let few = [c("a", 0, 1), c("b", 0, 1), c("c", 0, 1)];
        let b = ["b".to_string()];
        for (pool, query, exclude, want) in [
            (&near[..], "", &[][..], &["done", "idle", "in-review", "in-progress"][..]),
            (&near, "in", &[], &["in-review", "in-progress"]),
            (&word, "pro", &[], &["progress", "in-progress", "reprogram"]),
            (&few, "a", &b, &[]),
            (&few, "", &b, &["a", "c"]),
        ] {
            assert_eq!(texts(&rank(pool, query, exclude)), want, "{query:?}");
        }
    }

    /// Rule: the top candidate holds as the rest of it is typed.
    #[test]
    fn the_top_holds_while_the_rest_is_typed() {
        let pool = [c("foundry", 1, 2), c("fonts", 0, 5), c("foundation", 2, 1), c("fo", 3, 1)];
        // up to the whole word, which is no longer a completion
        let mut held = false;
        for end in 1.."foundry".len() {
            let q = &"foundry"[..end];
            let on_top = rank(&pool, q, &[])
                .first()
                .is_some_and(|r| r.text == "foundry");
            assert!(on_top || !held, "fell from the top at {q:?}");
            held = on_top;
        }
    }
}
