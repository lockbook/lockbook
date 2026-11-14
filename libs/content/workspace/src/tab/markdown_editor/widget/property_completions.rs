//! Completions in the front matter grid: property names while a key is
//! typed, and the values other notes gave that key while a value or list
//! item is typed. Candidates come from the note index; the nearest note
//! that uses one ranks it, so a folder's own vocabulary comes first.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use egui::{Context, Ui};
use lb_rs::Uuid;
use lb_rs::model::text::buffer::Buffer;
use lb_rs::model::text::offset_types::{Grapheme, RangeExt as _};

use crate::file_cache::{FileCache, FilesExt as _};
use crate::front_matter::{FrontMatter, ListStyle, Quotes, RowKind as Kind, Value};
use crate::tab::markdown_editor::MdEdit;
use crate::tab::markdown_editor::input::front_matter::yaml_scalar;
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
    /// What ends this text and starts the next, typed after the candidate
    /// when that character takes it: `: ` for a key being typed.
    next: Option<(char, String)>,
    /// A value after a bare `key:` needs a space before it.
    space: bool,
    /// Already in the block or the list; not offered again.
    exclude: Vec<String>,
    /// The quote a value is typed inside, which a candidate is escaped for.
    quote: Option<char>,
    /// A value is an item inside brackets.
    flow: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Candidate {
    pub text: String,
    /// Notes with it.
    pub count: usize,
    /// Folders to the nearest note with it.
    pub distance: usize,
}

/// What put the caret where it is, as the popup sees it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
enum Arrival {
    #[default]
    Moved,
    /// A click or Tab.
    Landed,
    /// Typing, deleting, or a new line.
    Typed,
}

#[derive(Default)]
pub struct PropertyCompletions {
    /// True when a key, value, or item is being typed and has candidates.
    /// Read by the editor to gate rendering; also gates `handle_input`.
    pub active: bool,
    /// Keyboard-highlighted result index; see [`Self::highlighted`].
    pub selected: usize,
    /// The highlight was moved with the arrows since the query changed.
    picked: bool,
    /// How the caret came to where it is. The popup opens on typing: the
    /// caret arriving by the arrows, or a command changing the row,
    /// leaves it shut, so Return and the arrows keep moving. A click or
    /// Tab landing on an empty value opens it, as does any arrival on
    /// touch, which has no such keys.
    arrival: Arrival,
    /// The arrival of the events applied since the last frame.
    pending: Arrival,
    /// The text's version and the selection as last seen.
    seen: Option<(u64, (Grapheme, Grapheme))>,
    /// What is typed is itself a candidate, so the character that ends it
    /// keeps it.
    typed_known: bool,
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
        files: &Arc<RwLock<FileCache>>, file_id: Uuid, text_seq: u64, touch: bool,
    ) {
        let selection = buffer.current.selection;
        if self.seen.is_some_and(|seen| seen != (text_seq, selection)) {
            self.arrival = self.pending;
        }
        self.seen = Some((text_seq, selection));
        self.pending = Arrival::Moved;
        let previous = self.target.take();
        self.active = false;
        self.search_term_range = None;
        self.results.clear();
        let Some(target) = fm.and_then(|fm| target(buffer, fm)) else { return };
        let same = previous.is_some_and(|p| {
            p.replace.start() == target.replace.start() && p.query == target.query
        });
        if !same {
            self.selected = 0;
            self.picked = false;
        }
        let arrived = (touch || self.arrival == Arrival::Landed) && target.query.is_empty();
        let armed = self.arrival == Arrival::Typed || arrived;
        if !armed || self.suppressed.as_deref() == Some(target.query.as_str()) {
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

    /// Notes an event the editor applies, which puts the caret where the
    /// next frame finds it.
    pub fn note(&mut self, event: &Event) {
        let arrival = match event {
            Event::Replace { .. } | Event::Delete { .. } | Event::Newline { .. } => Arrival::Typed,
            Event::Indent { .. }
            | Event::Select {
                region:
                    Region::Location(Location::Pos(_))
                    | Region::BetweenLocations { start: Location::Pos(_), .. },
            } => Arrival::Landed,
            _ => Arrival::Moved,
        };
        self.pending = self.pending.max(arrival);
    }

    /// The highlighted candidate: none until one is picked, or something
    /// is typed that isn't a candidate of its own — so Tab and Return move
    /// on from an empty value or a finished one.
    pub fn highlighted(&self) -> Option<usize> {
        let typed = self.target.as_ref().is_some_and(|t| !t.query.is_empty());
        (self.picked || typed && !self.typed_known).then_some(self.selected)
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
        self.typed_known = candidates
            .iter()
            .any(|c| c.text.to_lowercase() == target.query.to_lowercase());
        rank(candidates, &target.query, &target.exclude)
    }

    /// Consume (or observe) keyboard events targeting the popup. Emitted
    /// replacements are pushed onto `events`. Escape is observed, not
    /// consumed, and works unfocused; the rest only with focus. Up and
    /// Down (or Ctrl+P and Ctrl+N) pick, Return and Tab take the highlight,
    /// and the colon that ends a key takes it too when it was picked or
    /// begins with what is typed. A list item's comma ends it as typed.
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
        let last = self.results.len() - 1;
        if let Some(step) = completion_step(ctx) {
            let up = step == CompletionStep::Up;
            self.selected = match (self.highlighted(), up) {
                (None, true) => last,
                (None, false) => 0,
                (Some(s), true) => s.saturating_sub(1),
                (Some(s), false) => (s + 1).min(last),
            };
            self.picked = true;
        }
        let Some(selected) = self.highlighted() else { return };
        let text = self.results[selected].text.clone();
        if completion_accepted(ctx) {
            self.apply_completion(events, &target, &text, "");
            return;
        }
        let Some((end, next)) = &target.next else { return };
        let extends = text
            .to_lowercase()
            .starts_with(&target.query.to_lowercase());
        if !(self.picked || extends) {
            return;
        }
        let typed = ctx.input_mut(|i| {
            let at = i
                .events
                .iter()
                .position(|e| matches!(e, egui::Event::Text(t) if t.starts_with(*end)))?;
            let egui::Event::Text(t) = i.events.remove(at) else { unreachable!() };
            Some(t)
        });
        if let Some(typed) = typed {
            // anything typed with it goes after
            let rest = typed[end.len_utf8()..].to_string();
            self.apply_completion(events, &target, &text, &format!("{next}{rest}"));
        }
    }

    /// Replace the typed text with the candidate and reset popup state.
    fn apply_completion(
        &mut self, events: &mut Vec<Event>, target: &Target, text: &str, after: &str,
    ) {
        let text = match target.pool {
            Pool::Values(_) => yaml_scalar(text, target.quote, target.flow),
            Pool::Keys => text.into(),
        };
        let text = match (target.commit_key, target.space) {
            (true, _) => format!("{text}: "),
            (false, true) => format!(" {text}{after}"),
            (false, false) => format!("{text}{after}"),
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
        self.picked = false;
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

/// The key, value, or item the caret is at the end of, or the empty row
/// it is on, if any.
fn target(buffer: &Buffer, fm: &FrontMatter<Grapheme>) -> Option<Target> {
    let selection = buffer.current.selection;
    if !selection.is_empty() {
        return None;
    }
    let offset = selection.0;
    let i = fm.row_at(offset)?;
    let quote =
        |quotes: &Quotes<Grapheme>| quotes.and_then(|(open, _)| buffer[open].chars().next());
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
        // a new row's key, before anything is typed
        Kind::Blank if offset == fm.rows[i].line.start() => Some(Target {
            replace: (offset, offset),
            query: String::new(),
            pool: Pool::Keys,
            commit_key: true,
            next: Some((':', String::new())),
            space: false,
            exclude: keys.clone(),
            quote: None,
            flow: false,
        }),
        Kind::Pending { key } if offset == key.end() && !key.is_empty() => Some(Target {
            replace: *key,
            query: buffer[*key].to_string(),
            pool: Pool::Keys,
            commit_key: true,
            next: Some((':', String::new())),
            space: false,
            exclude: others(&buffer[*key]),
            quote: None,
            flow: false,
        }),
        Kind::Property { key, value, .. } => {
            if offset == key.end() && !key.is_empty() {
                return Some(Target {
                    replace: *key,
                    query: buffer[*key].to_string(),
                    pool: Pool::Keys,
                    commit_key: false,
                    next: None,
                    space: false,
                    exclude: others(&buffer[*key]),
                    quote: None,
                    flow: false,
                });
            }
            let pool = Pool::Values(buffer[*key].to_string());
            match value {
                Value::Text { text, checked: None, quotes } if offset == text.end() => {
                    Some(Target {
                        replace: *text,
                        query: buffer[*text].to_string(),
                        pool,
                        commit_key: false,
                        next: None,
                        space: false,
                        exclude: vec![],
                        quote: quote(quotes),
                        flow: false,
                    })
                }
                Value::Empty { at, needs_space } if offset == *at => Some(Target {
                    replace: (*at, *at),
                    query: String::new(),
                    pool,
                    commit_key: false,
                    next: None,
                    space: *needs_space,
                    exclude: vec![],
                    quote: None,
                    flow: false,
                }),
                Value::List { items, style, .. } => {
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
                        // a comma ends the item as typed; Return takes a candidate
                        next: None,
                        space: false,
                        exclude,
                        quote: quote(&item.quotes),
                        flow: *style == ListStyle::Flow,
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
        let highlighted = self.property_completions.highlighted();
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
                "",
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
