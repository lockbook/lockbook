use objc2_app_kit::NSSpellChecker;
use objc2_foundation::{NSInteger, NSNotFound, NSRange, NSString};

use super::SpellCheck;

/// The system spell checker, through AppKit.
pub struct AppKit;

/// No spell document: ignored words live in [`super::Spelling`].
const NO_DOCUMENT: NSInteger = 0;

impl SpellCheck for AppKit {
    fn check(&self, word: &str) -> Option<bool> {
        let word = NSString::from_str(word);
        let checker = NSSpellChecker::sharedSpellChecker();
        let mut count: NSInteger = 0;
        // SAFETY: `count` outlives the call.
        let misspelled = unsafe {
            checker.checkSpellingOfString_startingAt_language_wrap_inSpellDocumentWithTag_wordCount(
                &word,
                0,
                None,
                false,
                NO_DOCUMENT,
                &mut count,
            )
        };
        Some(misspelled.location == NSNotFound as usize)
    }

    fn suggest(&self, word: &str) -> Vec<String> {
        let string = NSString::from_str(word);
        let range = NSRange::new(0, string.length());
        NSSpellChecker::sharedSpellChecker()
            .guessesForWordRange_inString_language_inSpellDocumentWithTag(
                range,
                &string,
                None,
                NO_DOCUMENT,
            )
            .map(|guesses| guesses.iter().map(|g| g.to_string()).collect())
            .unwrap_or_default()
    }

    fn learn(&self, word: &str) {
        NSSpellChecker::sharedSpellChecker().learnWord(&NSString::from_str(word));
    }

    fn learned(&self, word: &str) -> bool {
        NSSpellChecker::sharedSpellChecker().hasLearnedWord(&NSString::from_str(word))
    }

    fn unlearn(&self, word: &str) {
        NSSpellChecker::sharedSpellChecker().unlearnWord(&NSString::from_str(word));
    }
}
