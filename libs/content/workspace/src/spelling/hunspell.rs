use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use egui::Context;
use spellbook::Dictionary;

use super::SpellCheck;

/// The Hunspell dictionary installed for the system's language, with
/// Hunspell's personal word list for it.
pub struct Hunspell {
    state: Arc<RwLock<State>>,
}

enum State {
    Loading,
    /// No dictionary is installed for the system's language.
    Missing,
    Ready {
        dictionary: Box<Dictionary>,
        personal: PathBuf,
    },
}

impl Hunspell {
    /// Loads off the UI thread; words are unchecked until it lands.
    pub fn load(ctx: Context) -> Self {
        let state = Arc::new(RwLock::new(State::Loading));
        let loading = state.clone();
        std::thread::spawn(move || {
            let state = ready().unwrap_or_else(|why| {
                tracing::warn!("spell check off: {why}");
                State::Missing
            });
            *loading.write().unwrap() = state;
            ctx.request_repaint();
        });
        Self { state }
    }
}

/// The loaded dictionary, or why there is none.
fn ready() -> Result<State, String> {
    let language = sys_locale::get_locale()
        .ok_or("no system locale")?
        .replace('-', "_");
    let dirs = dictionary_dirs();
    let (aff_path, dic_path) = dictionary_files(&dirs, &language)
        .ok_or_else(|| format!("no dictionary for locale {language} in {dirs:?}"))?;
    let read = |path: &Path| {
        std::fs::read_to_string(path).map_err(|e| format!("can't read {}: {e}", path.display()))
    };
    let (aff, dic) = (read(&aff_path)?, read(&dic_path)?);
    let mut dictionary = Dictionary::new(&aff, &dic)
        .map_err(|e| format!("can't parse {}: {e}", aff_path.with_extension("").display()))?;
    let home = std::env::var_os("HOME").ok_or("no HOME for the personal word list")?;
    let personal = PathBuf::from(home).join(format!(".hunspell_{language}"));
    for word in personal_words(&personal) {
        let _ = dictionary.add(&word);
    }
    tracing::info!("spell check on: locale {language}, dictionary {}", dic_path.display());
    Ok(State::Ready { dictionary: Box::new(dictionary), personal })
}

/// The words in Hunspell's personal word list at `path`.
fn personal_words(path: &Path) -> impl Iterator<Item = String> {
    let words = std::fs::read_to_string(path).unwrap_or_default();
    words
        .lines()
        .map(|w| w.trim().to_string())
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .into_iter()
}

/// Where the system's dictionaries may be.
fn dictionary_dirs() -> Vec<PathBuf> {
    let data_dirs =
        std::env::var("XDG_DATA_DIRS").unwrap_or_else(|_| "/usr/local/share:/usr/share".into());
    data_dirs
        .split(':')
        .flat_map(|d| ["hunspell", "myspell", "myspell/dicts"].map(|s| PathBuf::from(d).join(s)))
        .collect()
}

/// The `.aff` and `.dic` for `language` (`en_US`), or else for another
/// region of its language, from `dirs`.
fn dictionary_files(dirs: &[PathBuf], language: &str) -> Option<(PathBuf, PathBuf)> {
    let base = language.split('_').next().unwrap_or(language);
    let pair = |dir: &PathBuf, name: &str| {
        let (aff, dic) = (dir.join(format!("{name}.aff")), dir.join(format!("{name}.dic")));
        (aff.exists() && dic.exists()).then_some((aff, dic))
    };
    dirs.iter().find_map(|dir| pair(dir, language)).or_else(|| {
        dirs.iter().find_map(|dir| {
            let mut names: Vec<String> = std::fs::read_dir(dir)
                .ok()?
                .filter_map(|e| e.ok()?.path().file_stem()?.to_str().map(str::to_string))
                .filter(|n| n == base || n.starts_with(&format!("{base}_")))
                .collect();
            names.sort();
            names.iter().find_map(|name| pair(dir, name))
        })
    })
}

impl SpellCheck for Hunspell {
    fn check(&self, word: &str) -> Option<bool> {
        match &*self.state.read().unwrap() {
            State::Loading => None,
            State::Missing => Some(true),
            State::Ready { dictionary, .. } => Some(dictionary.check(word)),
        }
    }

    fn suggest(&self, word: &str) -> Vec<String> {
        let mut suggestions = Vec::new();
        if let State::Ready { dictionary, .. } = &*self.state.read().unwrap() {
            dictionary.suggest(word, &mut suggestions);
        }
        suggestions
    }

    fn learn(&self, word: &str) {
        if let State::Ready { dictionary, personal } = &mut *self.state.write().unwrap() {
            let _ = dictionary.add(word);
            if let Ok(mut file) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(personal)
            {
                let _ = writeln!(file, "{word}");
            }
        }
    }

    fn learned(&self, word: &str) -> bool {
        match &*self.state.read().unwrap() {
            State::Ready { personal, .. } => personal_words(personal).any(|w| w == word),
            _ => false,
        }
    }

    fn unlearn(&self, word: &str) {
        if let State::Ready { dictionary, personal } = &mut *self.state.write().unwrap() {
            dictionary.remove_stem(word);
            let kept: String = personal_words(personal)
                .filter(|w| w != word)
                .map(|w| format!("{w}\n"))
                .collect();
            let _ = std::fs::write(personal, kept);
        }
    }
}
