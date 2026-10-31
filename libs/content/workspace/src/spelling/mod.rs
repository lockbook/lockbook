//! Spell check. The platform's checker decides what is misspelled; the
//! editor decides what is prose and paints the marks.
//!
//! One [`Spelling`] per [`egui::Context`], made on first use from the
//! system checker. iOS and Android check words on their side: the host
//! takes [`Spelling::take_requests`] after a frame and gives
//! [`Spelling::answer`]s back.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use egui::{Context, Id};

#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

/// A platform spell checker, in the system's language and with the
/// system's learned words.
pub trait SpellCheck: Send + Sync {
    /// Whether `word` is spelled right; `None` until the checker can say.
    fn check(&self, word: &str) -> Option<bool>;
    fn suggest(&self, word: &str) -> Vec<String>;
    /// Adds `word` to the system's learned words.
    fn learn(&self, word: &str);
    /// Whether `word` is among the system's learned words.
    fn learned(&self, word: &str) -> bool;
    fn unlearn(&self, word: &str);
}

pub struct Spelling {
    /// `None` when the host checks words itself.
    checker: Option<Box<dyn SpellCheck>>,
    verdicts: Mutex<HashMap<String, Verdict>>,
    /// Words for the host to check, in the order they were asked about.
    requests: Mutex<Vec<String>>,
    /// Words the person ignored, until the app quits.
    ignored: Mutex<HashSet<String>>,
    /// Bumped by [`Spelling::recheck`]; a word asked in an older round is
    /// asked again the next time it is checked.
    round: Mutex<u64>,
}

struct Verdict {
    /// `None` while the host is checking it.
    correct: Option<bool>,
    round: u64,
}

/// Most suggestions a menu shows.
pub const MAX_SUGGESTIONS: usize = 5;

impl Spelling {
    pub fn new(checker: Box<dyn SpellCheck>) -> Self {
        Self::with(Some(checker))
    }

    /// Words are checked by the host through [`Self::take_requests`] and
    /// [`Self::answer`].
    pub fn host() -> Self {
        Self::with(None)
    }

    fn with(checker: Option<Box<dyn SpellCheck>>) -> Self {
        Self {
            checker,
            verdicts: Default::default(),
            requests: Default::default(),
            ignored: Default::default(),
            round: Default::default(),
        }
    }

    /// The platform's checker, if it has one.
    fn system(ctx: &Context) -> Option<Self> {
        let _ = ctx;
        #[cfg(target_os = "macos")]
        return Some(Self::new(Box::new(macos::AppKit)));
        #[cfg(windows)]
        return windows::Windows::new().map(|c| Self::new(Box::new(c)));
        #[allow(unreachable_code)]
        None
    }

    /// Whether `word` is spelled right; `None` until that is known.
    pub fn check(&self, word: &str) -> Option<bool> {
        if self.ignored.lock().unwrap().contains(word) {
            return Some(true);
        }
        let round = *self.round.lock().unwrap();
        let mut verdicts = self.verdicts.lock().unwrap();
        if let Some(checker) = &self.checker {
            if let Some(verdict) = verdicts.get(word) {
                return verdict.correct;
            }
            let correct = checker.check(word);
            if correct.is_some() {
                verdicts.insert(word.to_string(), Verdict { correct, round });
            }
            return correct;
        }
        if !verdicts.contains_key(word) {
            verdicts
                .insert(word.to_string(), Verdict { correct: None, round: round.wrapping_sub(1) });
        }
        let verdict = verdicts.get_mut(word).unwrap();
        if verdict.round != round {
            verdict.round = round;
            self.requests.lock().unwrap().push(word.to_string());
        }
        verdict.correct
    }

    /// Asks the host again about each word the next time it is checked,
    /// keeping its answer until the new one lands.
    pub fn recheck(&self) {
        *self.round.lock().unwrap() += 1;
    }

    pub fn suggest(&self, word: &str) -> Vec<String> {
        let mut suggestions = self
            .checker
            .as_ref()
            .map(|c| c.suggest(word))
            .unwrap_or_default();
        suggestions.truncate(MAX_SUGGESTIONS);
        suggestions
    }

    pub fn learn(&self, word: &str) {
        if let Some(checker) = &self.checker {
            checker.learn(word);
        }
        self.answer(word, true);
    }

    /// Whether the person taught the system `word`.
    pub fn learned(&self, word: &str) -> bool {
        self.checker.as_ref().is_some_and(|c| c.learned(word))
    }

    /// Takes `word` out of the system's learned words; it is checked again.
    pub fn unlearn(&self, word: &str) {
        if let Some(checker) = &self.checker {
            checker.unlearn(word);
        }
        self.verdicts.lock().unwrap().remove(word);
    }

    /// Accepts `word` until the app quits.
    pub fn ignore(&self, word: &str) {
        self.ignored.lock().unwrap().insert(word.to_string());
    }

    /// The words asked about since the last call, for the host to check.
    pub fn take_requests(&self) -> Vec<String> {
        std::mem::take(&mut *self.requests.lock().unwrap())
    }

    /// The host's verdict on `word`, or a word it learned.
    pub fn answer(&self, word: &str, correct: bool) {
        let round = *self.round.lock().unwrap();
        self.verdicts
            .lock()
            .unwrap()
            .entry(word.to_string())
            .or_insert(Verdict { correct: None, round })
            .correct = Some(correct);
    }
}

fn id() -> Id {
    Id::unique("lb_spelling")
}

/// The context's spell checker, made from the system's on first use.
pub fn get(ctx: &Context) -> Option<Arc<Spelling>> {
    if let Some(spelling) = ctx.data(|d| d.get_temp::<Option<Arc<Spelling>>>(id())) {
        return spelling;
    }
    let spelling = Spelling::system(ctx).map(Arc::new);
    ctx.data_mut(|d| d.insert_temp(id(), spelling.clone()));
    spelling
}

/// Replaces the context's spell checker.
pub fn install(ctx: &Context, spelling: Option<Spelling>) {
    ctx.data_mut(|d| d.insert_temp(id(), spelling.map(Arc::new)));
}

/// Whether a word in prose is one to check: it has a letter, and it is not a
/// tag or a mention.
pub fn checkable(word: &str, before: Option<char>) -> bool {
    word.chars().any(char::is_alphabetic) && !matches!(before, Some('#' | '@'))
}
