#if os(iOS)
    import Bridge
    import UIKit

    /// The system spell checker: it answers the words the editor asks about
    /// and offers fixes in the edit menu.
    enum Spelling {
        static let checker = UITextChecker()

        /// The checker's name for `mode`'s language (the keyboard's), or the
        /// closest one it has.
        static func language(_ mode: UITextInputMode?) -> String {
            let available = UITextChecker.availableLanguages
            let wanted = (mode?.primaryLanguage ?? Locale.preferredLanguages.first ?? "en_US")
                .replacingOccurrences(of: "-", with: "_")
            if available.contains(wanted) { return wanted }
            let base = wanted.split(separator: "_").first.map(String.init) ?? wanted
            return available.first { $0 == base || $0.hasPrefix(base + "_") } ?? "en_US"
        }

        /// Answers the words the editor asked about in `mode`'s language; true
        /// if one is misspelled.
        static func answer(_ wsHandle: UnsafeMutableRawPointer?, mode: UITextInputMode?) -> Bool {
            guard let requests = spelling_requests(wsHandle) else { return false }
            let words = String(cString: requests).split(separator: "\n").map(String.init)
            free_text(requests)
            let language = language(mode)
            var misspelled = false
            for word in words {
                let typo = isMisspelled(word, language: language)
                spelling_answer(wsHandle, word, !typo)
                misspelled = misspelled || typo
            }
            return misspelled
        }

        static func isMisspelled(_ word: String, language: String) -> Bool {
            let range = NSRange(location: 0, length: (word as NSString).length)
            return checker.rangeOfMisspelledWord(
                in: word, range: range, startingAt: 0, wrap: false, language: language
            ).location != NSNotFound
        }

        static func guesses(_ word: String, language: String) -> [String] {
            let range = NSRange(location: 0, length: (word as NSString).length)
            let guesses = checker.guesses(forWordRange: range, in: word, language: language) ?? []
            return Array(guesses.prefix(5))
        }
    }
#endif
