import Foundation
import Down
import AppKit

public protocol AttributeRange {
    var range: NSRange { get }
    var parent: AttributeRange? { get }
    var textSize: Int { get }
    var foreground: NSColor { get }
    var background: NSColor { get }
    var italics: Bool { get }
    var bold: Bool { get }
    var link: String? { get }
    var monospace: Bool { get }
    var indentation: Float { get }
}

extension AttributeRange {
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
        
        if indentation != 0 {
            let paraStyle = NSMutableParagraphStyle()
            paraStyle.firstLineHeadIndent = 7
            paraStyle.headIndent = CGFloat(indentation + 7)
            
            attrs[.paragraphStyle] = paraStyle
        }
        
        return attrs
    }
}

class BaseAR: AttributeRange {
    var range: NSRange
    var parent: AttributeRange?
    
    init(_ range: NSRange, _ parent: AttributeRange?) {
        self.range = range
        self.parent = parent
    }
    
    init(_ indexer: IndexConverter, _ node: Node, _ parent: AttributeRange?) {
        self.range = indexer.getRange(node)
        self.parent = parent
    }
        
    var textSize: Int { self.parent!.textSize }
    
    var foreground: NSColor { self.parent!.foreground }
    
    var background: NSColor { self.parent!.background }
    
    var italics: Bool { self.parent!.italics }
    
    var bold: Bool { self.parent!.bold }
    
    var link: String? { self.parent!.link }
    
    var monospace: Bool { self.parent!.monospace }
    
    var indentation: Float { self.parent!.indentation }
}

class DocumentAR: BaseAR {
    
    init(_ range: NSRange) { super.init(range, .none) }
        
    override var textSize: Int { 13 }
    
    override var foreground: NSColor { NSColor.labelColor }
    
    override var background: NSColor { NSColor.clear }
    
    override var italics: Bool { false }
    
    override var bold: Bool { false }
    
    override var link: String? { .none }
    
    override var monospace: Bool { false }
    
    override var indentation: Float { 0 }
}

class HeadingAR: BaseAR {
    private let headingLevel: Int
    
    init(_ indexer: IndexConverter, _ node: Heading, _ parent: AttributeRange?) {
        self.headingLevel = node.headingLevel
        super.init(indexer, node, parent)
    }
    
    override var textSize: Int { 26 - ((headingLevel - 1) * 2) }
    override var bold: Bool { headingLevel == 1 }
}

class InlineCodeAR: BaseAR {
    override var foreground: NSColor { NSColor.systemPink }
    override var monospace: Bool { true }
}

class CodeBlockAR: BaseAR {
    override var monospace: Bool { true }
    override var background: NSColor { NSColor.black.withAlphaComponent(0.65) }
    override var foreground: NSColor { NSColor.white }
}

class BlockQuoteAR: BaseAR {
    override var italics: Bool { true }
    override var foreground: NSColor { NSColor.secondaryLabelColor }
}

class StrongAR: BaseAR {
    override var bold: Bool { true }
}

class EmphasisAR: BaseAR {
    override var italics: Bool { true }
}

class LinkAR: BaseAR {
    private let destination: String?
    
    init(_ indexer: IndexConverter, _ node: Link, _ parent: AttributeRange?) {
        self.destination = node.url
        super.init(indexer, node, parent)
    }
    
    override var link: String? { destination }
}

class ItemAR: BaseAR {
    override var foreground: NSColor { NSColor.secondaryLabelColor }
}

class ParagraphAR: BaseAR {
    private var offset: Float = 0
    
    init(_ indexer: IndexConverter, _ node: Paragraph, _ parent: AttributeRange?) {
        super.init(indexer, node, parent)
    }
    
    init(_ indexer: IndexConverter, _ node: Paragraph, _ parent: ItemAR, _ startOfLine: NSString) {
        super.init(indexer, node, parent)
        // TODO maybe not what we actually want to do. Basically it seems that paragraph styles need
        // to apply to the first character to be taken seriously, this is a workaround for now
        self.range = indexer.getRange(
            startCol: 1,
            endCol: node.cmarkNode.pointee.end_column,
            startLine: node.cmarkNode.pointee.start_line,
            endLine: node.cmarkNode.pointee.end_line
        )
        self.offset = Float(startOfLine.size(withAttributes: parent.finalizeAttributes()).width)
        print(self.offset)
    }
    
    override var indentation: Float { offset }
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


