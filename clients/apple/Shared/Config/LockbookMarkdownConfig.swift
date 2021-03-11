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
    t.tintColor = .systemPink
    return t
} ()

func applyMarkdown(_ attr: NSMutableAttributedString, markdown: MarkdownNode) -> [NSAttributedString.Key : Any] {
    switch markdown.type {
    case .header:
        return [
            .font : systemFontWithTraits(headingTraits, fontSize*(10.0-CGFloat(markdown.headingLevel))/3),
            .foregroundColor : headingColor
        ]
    case .italic:
        return [
            .font : systemFontWithTraits(emphasisTraits)
        ]
    case .bold:
        return [
            .font : systemFontWithTraits(boldTraits)
        ]
    case .codeFence, .code:
        return [
            .font : codeFont,
        ]
    case .list:
        let paragraphStyle = NSMutableParagraphStyle()
        paragraphStyle.lineSpacing = 2.0
        return [
            .foregroundColor : lighterColor,
            .paragraphStyle : paragraphStyle
        ]
    case .quote:
        let paragraphStyle = NSMutableParagraphStyle()
        paragraphStyle.firstLineHeadIndent = 5.0
        return [
            .foregroundColor : lighterColor,
            .paragraphStyle : paragraphStyle
        ]
    }
}

func applyBody(_ attr: NSMutableAttributedString) -> [NSAttributedString.Key : Any] {
    return [
        NSAttributedString.Key.font : systemFont,
        NSAttributedString.Key.foregroundColor : textColor
    ]
}
