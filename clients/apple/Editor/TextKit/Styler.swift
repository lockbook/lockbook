import Foundation
import Down
import AppKit

public protocol AttributeRange {
    var range: NSRange { get }
    var parent: AttributeRange { get }
    var textSize: Int { get }
    var foreground: NSColor { get }
    var background: NSColor { get }
    var italics: Bool { get }
    var bold: Bool { get }
    var link: String? { get }
    var monospace: Bool { get }
}

extension AttributeRange {
    
    var textSize: Int { self.parent.textSize }
    
    var foreground: NSColor { self.parent.foreground }
    
    var background: NSColor { self.parent.background }
    
    var italics: Bool { self.parent.italics }
    
    var bold: Bool { self.parent.bold }
    
    var link: String? { self.parent.link }
    
    var monospace: Bool { self.parent.monospace }
    
    public func finalizeAttributes() -> [NSAttributedString.Key : Any] {
        var attrs: [NSAttributedString.Key : Any] = [
            .foregroundColor : self.foreground,
            .backgroundColor : self.background,
        ]
        
        if let l = link {
            attrs[.link] = l
        }
        
        var fontAttrs: NSFontDescriptor.SymbolicTraits = []
        if monospace { fontAttrs.insert(.monoSpace) }
        if bold { fontAttrs.insert(.bold) }
        if italics { fontAttrs.insert(.italic) }
        
        attrs[.font] = NSFont(
            descriptor: NSFont.systemFont(ofSize: CGFloat(textSize))
                .fontDescriptor
                .withSymbolicTraits(fontAttrs),
            size: CGFloat(textSize)
        )!
        
        return attrs
    }
}

// warning: if you miss a property below, the default extension will use parent,
// which is self, which will inf loop. Don't fuck up.
class DocumentAR: AttributeRange {
    
    var range: NSRange
    
    init(_ range: NSRange) { self.range = range }
    
    var parent: AttributeRange { self }
    
    var textSize: Int { 13 }
    
    var foreground: NSColor { NSColor.labelColor }
    
    var background: NSColor { NSColor.clear }
    
    var italics: Bool { false }
    
    var bold: Bool { false }
    
    var link: String? { .none }
    
    var monospace: Bool { false }
}

class HeadingAR: AttributeRange {
    
    var range: NSRange
    var parent: AttributeRange
    
    // TODO handle ranges
    init(_ range: NSRange, _ parent: AttributeRange) {
        self.range = range
        self.parent = parent
    }
    
    var textSize: Int { 26 }
    var bold: Bool { true }
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
}


