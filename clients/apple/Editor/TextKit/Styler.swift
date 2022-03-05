import Foundation
import Down
import AppKit

class Styler {
    
    static let body: [NSAttributedString.Key : Any] =
    [
        .foregroundColor : NSColor.labelColor,
        .font :  NSFont.systemFont(ofSize: baseFontSize),
    ]
    
    static let heading1: [NSAttributedString.Key : Any] =
    [
        .foregroundColor : headingColor,
        .font : NSFont(descriptor: NSFont.systemFont(ofSize: 30).fontDescriptor.withSymbolicTraits([.bold]), size: 30)!,
    ]
    
    static let heading2: [NSAttributedString.Key : Any] =
    [
        .foregroundColor : headingColor,
        .font : NSFont(descriptor: NSFont.systemFont(ofSize: 20).fontDescriptor.withSymbolicTraits([]), size: 20)!,
    ]
    
    static let heading3: [NSAttributedString.Key : Any] =
    [
        .foregroundColor : headingColor,
        .font : NSFont(descriptor: NSFont.systemFont(ofSize: 18).fontDescriptor.withSymbolicTraits([]), size: 18)!,
    ]
    
    static let heading4: [NSAttributedString.Key : Any] =
    [
        .foregroundColor : headingColor,
        .font : NSFont(descriptor: NSFont.systemFont(ofSize: 16).fontDescriptor.withSymbolicTraits([]), size: 16)!,
    ]
    
    static let heading5: [NSAttributedString.Key : Any] =
    [
        .foregroundColor : headingColor,
        .font : NSFont(descriptor: NSFont.systemFont(ofSize: 15).fontDescriptor.withSymbolicTraits([]), size: 15)!,
    ]
    
    static let heading6: [NSAttributedString.Key : Any] =
    [
        .foregroundColor : headingColor,
        .font : NSFont(descriptor: NSFont.systemFont(ofSize: 14).fontDescriptor.withSymbolicTraits([]), size: 14)!,
    ]
    
    static let code: [NSAttributedString.Key : Any] =
    [
        .foregroundColor : NSColor.systemPink,
        .font : NSFont.monospacedSystemFont(ofSize: baseFontSize, weight: .regular),
    ]
    
    static let codeBlock: [NSAttributedString.Key : Any] =
    [
        .foregroundColor : NSColor.windowBackgroundColor,
        .backgroundColor : NSColor.labelColor,
        .font : NSFont.monospacedSystemFont(ofSize: baseFontSize, weight: .regular),
    ]
    
    static let link: [NSAttributedString.Key : Any] =
    [
        .foregroundColor : NSColor.linkColor,
        .link : "https://lockbook.net",
    ]
    
    static let headingColor = NSColor.labelColor
    static let baseFontSize = CGFloat(13)
    
    static func style(_ style: Style) -> [NSAttributedString.Key : Any] {
        switch style {
        case .Base:
            return body
        case .Heading1:
            return heading1
        case .Heading2:
            return heading2
        case .Heading3:
            return heading3
        case .Heading4:
            return heading4
        case .Heading5:
            return heading5
        case .Heading6:
            return heading6
        case .Code:
            return code
        case .CodeBlock:
            return codeBlock
        case .Link:
            return link
        }
    }
}

enum Style {
    case Base
    case Heading1
    case Heading2
    case Heading3
    case Heading4
    case Heading5
    case Heading6
    case Code
    case CodeBlock
    case Link
}

extension Style {
    static func from(_ heading: Heading) -> Style {
        switch heading.headingLevel {
        case 1: return .Heading1
        case 2: return .Heading2
        case 3: return .Heading3
        case 4: return .Heading4
        case 5: return .Heading5
        default: return .Heading6
        }
    }
    
    static func from(_ document: Document) -> Style {
        .Base
    }
    
    static func from(_ code: Code) -> Style {
        .Code
    }
    
    static func from(_ code: CodeBlock) -> Style {
        .CodeBlock
    }
    
    static func from(_ code: Link) -> Style {
        .Link
    }
    
    func attributes() -> [NSAttributedString.Key : Any] {
        Styler.style(self)
    }
}


