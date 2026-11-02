use std::collections::{BTreeSet, HashSet};
use std::path::PathBuf;
use std::sync::Mutex;

use windows::Win32::Globalization::{
    GetUserDefaultLocaleName, GetUserPreferredUILanguages, ISpellChecker, ISpellChecker2,
    ISpellCheckerFactory, MUI_LANGUAGE_NAME, SpellCheckerFactory,
};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree,
};
use windows::core::{AgileReference, HSTRING, Interface, PWSTR};

use super::SpellCheck;

/// Win32's `LOCALE_NAME_MAX_LENGTH`, in UTF-16 units with the terminator.
const LOCALE_NAME_MAX_LENGTH: usize = 85;

/// The system spell checker, through the Windows spell checking API. Our
/// list of learned words decides over the system's, which roams and lags;
/// learning and unlearning reach both.
pub struct Windows {
    checker: AgileReference<ISpellChecker>,
    /// Where `learned` is kept, a word a line.
    file: Option<PathBuf>,
    learned: Mutex<BTreeSet<String>>,
    /// Words unlearned since launch; the checker may still accept them for
    /// a moment.
    unlearned: Mutex<HashSet<String>>,
}

impl Windows {
    pub fn new() -> Option<Self> {
        // SAFETY: plain COM calls; each language outlives its use.
        let checker = unsafe {
            // the UI thread is usually initialized already, which is fine
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
            let factory: ISpellCheckerFactory =
                CoCreateInstance(&SpellCheckerFactory, None, CLSCTX_INPROC_SERVER).ok()?;
            let language = languages().into_iter().find(|l| {
                factory
                    .IsSupported(&HSTRING::from(l.as_str()))
                    .is_ok_and(|s| s.as_bool())
            })?;
            factory.CreateSpellChecker(&HSTRING::from(language)).ok()?
        };
        let file = learned_file();
        let learned = file
            .as_ref()
            .and_then(|f| std::fs::read_to_string(f).ok())
            .unwrap_or_default()
            .lines()
            .map(str::trim)
            .filter(|w| !w.is_empty())
            .map(str::to_string)
            .collect();
        Some(Self {
            checker: AgileReference::new(&checker).ok()?,
            file,
            learned: Mutex::new(learned),
            unlearned: Default::default(),
        })
    }

    fn save(&self, learned: &BTreeSet<String>) {
        let Some(file) = &self.file else { return };
        let text: String = learned.iter().map(|w| format!("{w}\n")).collect();
        let temp = file.with_extension("tmp");
        let _ = std::fs::create_dir_all(file.parent().unwrap_or(file))
            .and_then(|_| std::fs::write(&temp, text))
            .and_then(|_| std::fs::rename(&temp, file));
    }
}

/// The languages to check in, best first: the person's display languages,
/// then the language of their regional format.
fn languages() -> Vec<String> {
    let mut languages = Vec::new();
    // SAFETY: the buffer is sized by the first call.
    unsafe {
        let (mut count, mut len) = (0, 0);
        if GetUserPreferredUILanguages(MUI_LANGUAGE_NAME, &mut count, None, &mut len).is_ok() {
            let mut buffer = vec![0u16; len as usize];
            if GetUserPreferredUILanguages(
                MUI_LANGUAGE_NAME,
                &mut count,
                Some(PWSTR(buffer.as_mut_ptr())),
                &mut len,
            )
            .is_ok()
            {
                // names separated by nulls, ending in two
                languages.extend(
                    buffer
                        .split(|&c| c == 0)
                        .filter(|l| !l.is_empty())
                        .map(String::from_utf16_lossy),
                );
            }
        }
        let mut locale = [0u16; LOCALE_NAME_MAX_LENGTH];
        let len = GetUserDefaultLocaleName(&mut locale);
        if len > 1 {
            languages.push(String::from_utf16_lossy(&locale[..len as usize - 1]));
        }
    }
    languages
}

/// Our list of learned words.
fn learned_file() -> Option<PathBuf> {
    Some(
        PathBuf::from(std::env::var_os("APPDATA")?)
            .join("Lockbook")
            .join("spelling")
            .join("learned.txt"),
    )
}

impl SpellCheck for Windows {
    fn check(&self, word: &str) -> Option<bool> {
        if self.learned.lock().unwrap().contains(word) {
            return Some(true);
        }
        if self.unlearned.lock().unwrap().contains(word) {
            return Some(false);
        }
        let checker = self.checker.resolve().ok()?;
        // SAFETY: plain COM calls.
        unsafe {
            let errors = checker.Check(&HSTRING::from(word)).ok()?;
            let mut error = None;
            errors.Next(&mut error).ok().ok()?;
            Some(error.is_none())
        }
    }

    fn suggest(&self, word: &str) -> Vec<String> {
        let mut suggestions = Vec::new();
        let Ok(checker) = self.checker.resolve() else { return suggestions };
        // SAFETY: each fetched string is the caller's to free.
        unsafe {
            let Ok(found) = checker.Suggest(&HSTRING::from(word)) else { return suggestions };
            loop {
                let mut next = [PWSTR::null()];
                let mut fetched = 0;
                if found.Next(&mut next, Some(&mut fetched)).is_err() || fetched == 0 {
                    break;
                }
                if let Ok(suggestion) = next[0].to_string() {
                    suggestions.push(suggestion);
                }
                CoTaskMemFree(Some(next[0].0 as _));
            }
        }
        suggestions
    }

    fn learn(&self, word: &str) {
        self.unlearned.lock().unwrap().remove(word);
        let mut learned = self.learned.lock().unwrap();
        if learned.insert(word.to_string()) {
            self.save(&learned);
        }
        if let Ok(checker) = self.checker.resolve() {
            // SAFETY: plain COM call.
            let _ = unsafe { checker.Add(&HSTRING::from(word)) };
        }
    }

    fn learned(&self, word: &str) -> bool {
        self.learned.lock().unwrap().contains(word)
    }

    fn unlearn(&self, word: &str) {
        self.unlearned.lock().unwrap().insert(word.to_string());
        let mut learned = self.learned.lock().unwrap();
        if learned.remove(word) {
            self.save(&learned);
        }
        if let Ok(checker) = self
            .checker
            .resolve()
            .and_then(|c| c.cast::<ISpellChecker2>())
        {
            // SAFETY: plain COM call.
            let _ = unsafe { checker.Remove(&HSTRING::from(word)) };
        }
    }
}
