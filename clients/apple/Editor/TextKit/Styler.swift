import Foundation
import Down

#if os(iOS)
import UIKit
#elseif os(macOS)
import AppKit
#endif

public protocol AttributeRange {
    var range: NSRange { get }
    var parent: AttributeRange? { get }
    var textSize: Int { get }
    var foreground: UniversalColor { get }
    var background: UniversalColor { get }
    var italics: Bool { get }
    var bold: Bool { get }
    var link: String? { get }
    var monospace: Bool { get }
    var indentation: Float { get }
    func isEqual (to: AttributeRange) -> Bool
}

extension AttributeRange where Self : Equatable {
    func isEqual (to: AttributeRange) -> Bool {
        return (to as? Self).flatMap({ $0 == self }) ?? false
    }
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
        
        #if os(iOS)
        var fontAttrs: UIFontDescriptor.SymbolicTraits = []
        if monospace { fontAttrs.insert(.traitMonoSpace) }
        if bold { fontAttrs.insert(.traitBold) }
        if italics { fontAttrs.insert(.traitItalic) }
        
        attrs[.font] = UIFont(
            descriptor: UIFont.systemFont(ofSize: CGFloat(textSize))
                .fontDescriptor
                .withSymbolicTraits(fontAttrs)!,
            size: CGFloat(textSize)
        )
        #else
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
        #endif

        
        if indentation != 0 {
            let paraStyle = NSMutableParagraphStyle()
            paraStyle.headIndent = CGFloat(indentation)
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
    
    var foreground: UniversalColor { self.parent!.foreground }
    
    var background: UniversalColor { self.parent!.background }
    
    var italics: Bool { self.parent!.italics }
    
    var bold: Bool { self.parent!.bold }
    
    var link: String? { self.parent!.link }
    
    var monospace: Bool { self.parent!.monospace }
    
    var indentation: Float { self.parent!.indentation }
    
    func isEqual(to: AttributeRange) -> Bool {
        self.textSize == to.textSize &&
        self.foreground == to.foreground &&
        self.background == to.background &&
        self.italics == to.italics &&
        self.bold == to.bold &&
        self.link == to.link &&
        self.monospace == to.monospace &&
        self.indentation == to.indentation
    }
}

class DocumentAR: BaseAR {
    
    init(_ range: NSRange) { super.init(range, .none) }
    
    override var textSize: Int { 13 }
    
    override var foreground: UniversalColor { UniversalColor.label }
    
    override var background: UniversalColor { UniversalColor.clear }
    
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
    override var foreground: UniversalColor { UniversalColor.systemPink }
    override var monospace: Bool { true }
}

class CodeBlockAR: BaseAR {
    override var monospace: Bool { true }
    override var background: UniversalColor { UniversalColor.black.withAlphaComponent(0.65) }
    override var foreground: UniversalColor { UniversalColor.white }
}

class BlockQuoteAR: BaseAR {
    override var italics: Bool { true }
    override var foreground: UniversalColor { UniversalColor.secondaryLabel }
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
    override var foreground: UniversalColor { UniversalColor.secondaryLabel }
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
    }
    
    init(_ indexer: IndexConverter, _ node: Item, _ parent: ItemAR, _ startOfLine: NSString) {
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
    }
    
    init(_ indexer: IndexConverter, _ node: Heading, _ parent: ItemAR, _ startOfLine: NSString) {
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
    }
    
    override var indentation: Float { offset }
}

class TextAR: BaseAR { }
