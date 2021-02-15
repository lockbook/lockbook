import SwiftUI
import Foundation
import Sourceful

fileprivate let inlineCodeRegex = try! NSRegularExpression(pattern: "`[^`]*`", options: [])
fileprivate let codeBlockRegex = try! NSRegularExpression(pattern: "(`){3}((?!\\1).)+\\1{3}", options: [.dotMatchesLineSeparators])
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
let codeFont = NSFont.monospacedSystemFont(ofSize: fontSize, weight: .thin)
let headingTraits: NSFontDescriptor.SymbolicTraits = [.bold, .expanded]
let boldTraits: NSFontDescriptor.SymbolicTraits = [.bold]
let emphasisTraits: NSFontDescriptor.SymbolicTraits = [.italic]
let boldEmphasisTraits: NSFontDescriptor.SymbolicTraits = [.bold, .italic]
let secondaryBackground = NSColor.windowBackgroundColor
let lighterColor = NSColor.lightGray
let textColor = NSColor.labelColor
let headingColor = NSColor(red: 0.94, green: 0.51, blue: 0.69, alpha: 1)
func systemFontWithTraits(_ traits: NSFontDescriptor.SymbolicTraits) -> NSFont {
    NSFont(descriptor: NSFont.systemFont(ofSize: fontSize).fontDescriptor.withSymbolicTraits(traits), size: fontSize)!
}

#else
let fontSize = UIFont.systemFontSize
let codeFont = UIFont.monospacedSystemFont(ofSize: fontSize, weight: .thin)
let headingTraits: UIFontDescriptor.SymbolicTraits = [.traitBold, .traitExpanded]
let boldTraits: UIFontDescriptor.SymbolicTraits = [.traitBold]
let emphasisTraits: UIFontDescriptor.SymbolicTraits = [.traitItalic]
let boldEmphasisTraits: UIFontDescriptor.SymbolicTraits = [.traitBold, .traitItalic]
let secondaryBackground = UIColor.secondarySystemBackground
let lighterColor = UIColor.lightGray
let textColor = UIColor.label
let headingColor = UIColor(red: 0.94, green: 0.51, blue: 0.69, alpha: 1)
func systemFontWithTraits(_ traits: UIFontDescriptor.SymbolicTraits) -> UIFont {
    UIFont(descriptor: UIFont.systemFont(ofSize: fontSize).fontDescriptor.withSymbolicTraits(traits)!, size: fontSize)
}

#endif



struct LockbookSourceCodeTheme: SourceCodeTheme {
    let isDark: Bool

    var lineNumbersStyle: LineNumbersStyle? = .none

    var gutterStyle: GutterStyle = .init(backgroundColor: Sourceful.Color.white, minimumWidth: 0)

    var font: Sourceful.Font = Sourceful.Font(name: "Menlo", size: fontSize)!

    var backgroundColor: Sourceful.Color = .init(red: 0, green: 0, blue: 0, alpha: 0)

    func globalAttributes() -> [NSAttributedString.Key : Any] {
        var attributes = [NSAttributedString.Key: Any]()
        attributes[.font] = font
        attributes[.foregroundColor] = isDark ? Sourceful.Color.white : Sourceful.Color.black
        return attributes
    }

    func attributes(for token: Token) -> [NSAttributedString.Key : Any] {
        var attributes = [NSAttributedString.Key: Any]()
        if let token = token as? MarkdownToken {
            switch token.type {
            case .plain:
                attributes[.font] = font
                attributes[.foregroundColor] = Sourceful.Color.white
                break
            case .inlineCode:
                attributes[.font] = codeFont
                attributes[.foregroundColor] = Sourceful.Color.gray
                break
            case .codeBlock:
                attributes[.font] = codeFont
                attributes[.foregroundColor] = Sourceful.Color.gray
                break
            case .heading:
                attributes[.font] = systemFontWithTraits(headingTraits)
                attributes[.foregroundColor] = headingColor
                break
            }
        }
        return attributes
    }

    func color(for syntaxColorType: SourceCodeTokenType) -> Sourceful.Color {
        return Sourceful.Color.systemPink
    }
}

struct LockbookMarkdownLexer: SourceCodeRegexLexer {
    private func fromRegex(_ regex: NSRegularExpression, _ tokenType: MarkdownTokenType) -> TokenGenerator {
        TokenGenerator.regex(RegexTokenGenerator(regularExpression: regex, tokenTransformer: { range -> Token in
            SimpleMarkdownToken(tokenType, range)
        }))
    }

    func generators(source: String) -> [TokenGenerator] {
        [
            fromRegex(inlineCodeRegex, .inlineCode),
            fromRegex(codeBlockRegex, .codeBlock),
            fromRegex(headingRegex, .heading),
            //            fromRegex(linkOrImageRegex, .plain),
            //            fromRegex(linkOrImageTagRegex, .plain),
            //            fromRegex(boldRegex, .plain),
            //            fromRegex(underscoreEmphasisRegex, .plain),
            //            fromRegex(asteriskEmphasisRegex, .plain),
            //            fromRegex(boldEmphasisAsteriskRegex, .plain),
            //            fromRegex(blockquoteRegex, .plain),
            //            fromRegex(horizontalRuleRegex, .plain),
            //            fromRegex(unorderedListRegex, .plain),
            //            fromRegex(orderedListRegex, .plain),
            //            fromRegex(buttonRegex, .plain),
            //            fromRegex(strikethroughRegex, .plain),
            //            fromRegex(tagRegex, .plain),
            //            fromRegex(footnoteRegex, .plain),
            //            fromRegex(htmlRegex, .plain),
        ]
    }
}

struct EmptyMarkdownLexer: SourceCodeRegexLexer {
    func generators(source: String) -> [TokenGenerator] {
        []
    }
}

enum MarkdownTokenType {
    case plain
    case inlineCode
    case codeBlock
    case heading

}

protocol MarkdownToken: Token {
    var type: MarkdownTokenType { get }
}

struct SimpleMarkdownToken: MarkdownToken {
    let isEditorPlaceholder: Bool = false

    let isPlain: Bool

    let type: MarkdownTokenType

    let range: Range<String.Index>

    init(_ tokenType: MarkdownTokenType, _ range: Range<String.Index>) {
        self.type = tokenType
        self.range = range
        self.isPlain = type == .plain
    }
}
