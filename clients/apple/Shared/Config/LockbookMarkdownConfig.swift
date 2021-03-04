import SwiftUI
import Foundation
import NotepadSwift

fileprivate let inlineCodeRegex = try! NSRegularExpression(pattern: "`[^`]*`", options: [])
fileprivate let codeBlockRegex = try! NSRegularExpression(pattern: "```\n.+\n```", options: [.dotMatchesLineSeparators])
fileprivate let headingRegex = try! NSRegularExpression(pattern: "^#{1,6}\\s.*$", options: [.anchorsMatchLines])
fileprivate let linkOrImageRegex = try! NSRegularExpression(pattern: "!?\\[([^\\[\\]]*)\\]\\((.*?)\\)", options: [])
fileprivate let linkOrImageTagRegex = try! NSRegularExpression(pattern: "!?\\[([^\\[\\]]*)\\]\\[(.*?)\\]", options: [])
fileprivate let boldRegex = try! NSRegularExpression(pattern: "((\\*|_){2})((?!\\1).)+\\1", options: [])
fileprivate let underscoreEmphasisRegex = try! NSRegularExpression(pattern: "(?<!_)_[^_]+_(?!\\*)", options: [])
fileprivate let asteriskEmphasisRegex = try! NSRegularExpression(pattern: "(?<!\\*)(\\*)((?!\\1).)+\\1(?!\\*)", options: [])
fileprivate let boldEmphasisAsteriskRegex = try! NSRegularExpression(pattern: "(\\*){3}((?!\\1).)+\\1{3}", options: [])
fileprivate let blockquoteRegex = try! NSRegularExpression(pattern: "^>.*", options: [.anchorsMatchLines])
fileprivate let horizontalRuleRegex = try! NSRegularExpression(pattern: "\n\n(-{3}|\\*{3})\n", options: [])
fileprivate let unorderedListRegex = try! NSRegularExpression(pattern: "^(\\-|\\*)\\s", options: [.anchorsMatchLines])
fileprivate let orderedListRegex = try! NSRegularExpression(pattern: "^\\d*\\.\\s", options: [.anchorsMatchLines])
fileprivate let buttonRegex = try! NSRegularExpression(pattern: "<\\s*button[^>]*>(.*?)<\\s*/\\s*button>", options: [])
fileprivate let strikethroughRegex = try! NSRegularExpression(pattern: "(~)((?!\\1).)+\\1", options: [])
fileprivate let tagRegex = try! NSRegularExpression(pattern: "^\\[([^\\[\\]]*)\\]:", options: [.anchorsMatchLines])
fileprivate let footnoteRegex = try! NSRegularExpression(pattern: "\\[\\^(.*?)\\]", options: [])
// courtesy https://www.regular-expressions.info/examples.html
fileprivate let htmlRegex = try! NSRegularExpression(pattern: "<([A-Z][A-Z0-9]*)\\b[^>]*>(.*?)</\\1>", options: [.dotMatchesLineSeparators, .caseInsensitive])

#if os(macOS)
let fontSize = NSFont.systemFontSize
let systemFont = NSFont.systemFont(ofSize: fontSize)
let codeFont = NSFont.monospacedSystemFont(ofSize: fontSize, weight: .thin)
let headingTraits: NSFontDescriptor.SymbolicTraits = [.bold, .expanded]
let boldTraits: NSFontDescriptor.SymbolicTraits = [.bold]
let emphasisTraits: NSFontDescriptor.SymbolicTraits = [.italic]
let boldEmphasisTraits: NSFontDescriptor.SymbolicTraits = [.bold, .italic]
let secondaryBackground = NSColor.windowBackgroundColor
let lighterColor = NSColor.lightGray
let textColor = NSColor.labelColor
let headingColor = NSColor(red: 0.94, green: 0.51, blue: 0.69, alpha: 1)
func systemFontWithTraits(_ traits: NSFontDescriptor.SymbolicTraits, _ size: CGFloat = fontSize) -> NSFont {
    NSFont(descriptor: NSFont.systemFont(ofSize: size).fontDescriptor.withSymbolicTraits(traits), size: size)!
}

#else
let fontSize = UIFont.systemFontSize
let systemFont = UIFont.systemFont(ofSize: fontSize)
let codeFont = UIFont.monospacedSystemFont(ofSize: fontSize, weight: .thin)
let headingTraits: UIFontDescriptor.SymbolicTraits = [.traitBold, .traitExpanded]
let boldTraits: UIFontDescriptor.SymbolicTraits = [.traitBold]
let emphasisTraits: UIFontDescriptor.SymbolicTraits = [.traitItalic]
let boldEmphasisTraits: UIFontDescriptor.SymbolicTraits = [.traitBold, .traitItalic]
let secondaryBackground = UIColor.secondarySystemBackground
let lighterColor = UIColor.lightGray
let textColor = UIColor.label
let headingColor = UIColor(red: 0.94, green: 0.51, blue: 0.69, alpha: 1)
func systemFontWithTraits(_ traits: UIFontDescriptor.SymbolicTraits, _ size: CGFloat = fontSize) -> UIFont {
    UIFont(descriptor: UIFont.systemFont(ofSize: size).fontDescriptor.withSymbolicTraits(traits)!, size: size)
}

#endif
let LockbookTheme: Theme = {
    var t = Theme()
//    t.backgroundColor = .init(white: 0, alpha: 0)
    t.tintColor = .systemPink
    t.body = Style(element: .body, attributes: [
        NSAttributedString.Key.font : systemFont,
        NSAttributedString.Key.foregroundColor : textColor
    ])
    t.styles = [
        Style(element: .bold, attributes: [
            NSAttributedString.Key.font : systemFontWithTraits(boldTraits)
        ]),
        Style(element: .italic, attributes: [
            NSAttributedString.Key.font : systemFontWithTraits(emphasisTraits)
        ]),
        Style(element: .boldItalic, attributes: [
            NSAttributedString.Key.font : systemFontWithTraits(boldEmphasisTraits)
        ]),
        Style(element: .h1, attributes: [
            NSAttributedString.Key.font : systemFontWithTraits(headingTraits, fontSize*5/3),
            NSAttributedString.Key.foregroundColor : headingColor
        ]),
        Style(element: .h2, attributes: [
            NSAttributedString.Key.font : systemFontWithTraits(headingTraits, fontSize*4/3),
            NSAttributedString.Key.foregroundColor : headingColor
        ]),
        Style(element: .h3, attributes: [
            NSAttributedString.Key.font : systemFontWithTraits(headingTraits),
            NSAttributedString.Key.foregroundColor : headingColor
        ]),
        Style(element: .code, attributes: [
            NSAttributedString.Key.font : codeFont,
        ]),
    ]
    return t
} ()
