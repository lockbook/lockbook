import SwiftUI
import Foundation
import NotepadSwift

#if os(macOS)
let fontSize = NSFont.systemFontSize
let systemFont = NSFont.systemFont(ofSize: fontSize)
let codeFont = NSFont.monospacedSystemFont(ofSize: fontSize, weight: .thin)
let headingTraits: NSFontDescriptor.SymbolicTraits = [.bold, .expanded]
let boldTraits: NSFontDescriptor.SymbolicTraits = [.bold]
let emphasisTraits: NSFontDescriptor.SymbolicTraits = [.italic]
let boldEmphasisTraits: NSFontDescriptor.SymbolicTraits = [.bold, .italic]
func systemFontWithTraits(_ traits: NSFontDescriptor.SymbolicTraits, _ size: CGFloat = fontSize) -> NSFont {
    NSFont(descriptor: NSFont.systemFont(ofSize: size).fontDescriptor.withSymbolicTraits(traits), size: size)!
}

#else
let fontSize = UIFont.systemFontSize
let systemFont = UIFont.systemFont(ofSize: fontSize)
let codeFont = systemFontWithTraits(.traitMonoSpace)
let headingTraits: UIFontDescriptor.SymbolicTraits = [.traitBold, .traitExpanded]
let boldTraits: UIFontDescriptor.SymbolicTraits = [.traitBold]
let emphasisTraits: UIFontDescriptor.SymbolicTraits = [.traitItalic]
let boldEmphasisTraits: UIFontDescriptor.SymbolicTraits = [.traitBold, .traitItalic]
func systemFontWithTraits(_ traits: UIFontDescriptor.SymbolicTraits, _ size: CGFloat = fontSize) -> UIFont {
    UIFont(descriptor: UIFont.systemFont(ofSize: size).fontDescriptor.withSymbolicTraits(traits)!, size: size)
}

#endif
let LockbookTheme: Theme = {
    var t = Theme()
    t.tintColor = UniversalColor.fromColorAlias(from: .Red)
    return t
} ()

func applyMarkdown(markdown: MarkdownNode) -> [NSAttributedString.Key : Any] {
    switch markdown.type {
    case .header:
        return [
            .foregroundColor : UniversalColor.fromColorAlias(from: .Red),
            .font : systemFontWithTraits(headingTraits, fontSize*(10.0-CGFloat(markdown.headingLevel))/3),
        ]
    case .italic:
        return [
            .font : systemFontWithTraits(emphasisTraits),
        ]
    case .bold:
        return [
            .font : systemFontWithTraits(boldTraits),
        ]
    case .codeFence, .code:
        return [
            .font : codeFont,
            .backgroundColor : UniversalColor.secondarySystemBackground,
        ]
    case .list:
        let paragraphStyle = NSMutableParagraphStyle()
        paragraphStyle.lineSpacing = 2.0
        return [
            .foregroundColor : UniversalColor.systemGray,
            .paragraphStyle : paragraphStyle,
        ]
    case .quote:
        let paragraphStyle = NSMutableParagraphStyle()
        paragraphStyle.firstLineHeadIndent = 5.0
        return [
            .foregroundColor : UniversalColor.fromColorAlias(from: .Magenta),
            .paragraphStyle : paragraphStyle,
        ]
    }
}

func applyBody() -> [NSAttributedString.Key : Any] {
    return [
        .foregroundColor : UniversalColor.label,
        .font : systemFont,
    ]
}
