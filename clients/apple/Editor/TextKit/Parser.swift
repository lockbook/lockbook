import Foundation
import Down

public class Parser: Visitor {

    public var indexes: IndexConverter
    public var typeAssist: TypeAssist
    public var processedDocument: [AttributeRange] = []
    var currentParent: AttributeRange?
    let input: NSString

    public init(_ input: String) {
        self.indexes = IndexConverter(input)
        let startingPoint = Date()
        let document = (try? Down(markdownString: input).toDocument())!
        print("Down perf: \(startingPoint.timeIntervalSinceNow * -1)")

        self.input = input as NSString
        self.typeAssist = TypeAssist(indexes)
        self.visit(document: document)
    }

    public func visit(document node: Document)  {
        print("Document start line: \(node.cmarkNode.pointee.start_line), endline: \(node.cmarkNode.pointee.end_line), start_column: \(node.cmarkNode.pointee.start_column), end_column: \(node.cmarkNode.pointee.end_column)")

        let doc = DocumentAR(indexes.getRange(node))
        self.currentParent = doc
        processedDocument.append(doc)
        let _ = visitChildren(of: node)
    }

    public func visit(blockQuote node: BlockQuote)  {
        let oldParent = self.currentParent
        let newParent = BlockQuoteAR(indexes, node, currentParent!)
        self.currentParent = newParent
        processedDocument.append(newParent)
        let _ = visitChildren(of: node)
        self.currentParent = oldParent
    }

    public func visit(list node: List)  {
        let _ = visitChildren(of: node)
    }

    public func visit(item node: Item)  {
        let oldParent = self.currentParent
        let newParent = ItemAR(indexes, node, currentParent!)
        self.currentParent = newParent
        processedDocument.append(newParent)
        // Items without any children yet will not be properly styled. So an edge case is added here in addition to paragraph
        // There are further edge cases with items that have a soft break inside them. It is perhaps possible to cleanup a
        // bunch of code by thinking through paragraph styling from an item centric perspective rather than a paragraph centric
        // one. The paragraph centric approach makes it easy to calculate where the bullet / number ends and the content begins.
        if node.children.isEmpty {
            let itemDefinition = indexes.getRange(
                startCol: 1,
                endCol: node.cmarkNode.pointee.end_column,
                startLine: node.cmarkNode.pointee.start_line,
                endLine: node.cmarkNode.pointee.start_line
            )

            let startOfLine = self.input.substring(with: itemDefinition)
            let dummyPara = ParagraphAR(indexes, node, newParent, startOfLine as NSString)
            processedDocument.append(dummyPara)
            typeAssist.nodeOfInterest(nodeRange: newParent.range, startOfLine, lineStartRange: itemDefinition, fresh: true)
        } else {
            let _ = visitChildren(of: node)
        }
        
        self.currentParent = oldParent
    }

    public func visit(codeBlock node: CodeBlock)  {
        let oldParent = self.currentParent
        let newParent = CodeBlockAR(indexes, node, currentParent!)
        self.currentParent = newParent
        processedDocument.append(newParent)
        let _ = visitChildren(of: node)
        self.currentParent = oldParent
    }

    public func visit(htmlBlock node: HtmlBlock)  {
        let _ = visitChildren(of: node)
    }

    public func visit(customBlock node: CustomBlock)  {
        let _ = visitChildren(of: node)
    }

    public func visit(paragraph node: Paragraph)  {
        print("para")
        let oldParent = self.currentParent
        var newParent: ParagraphAR
        if let itemParent = oldParent as? ItemAR {
            let itemDefinition = indexes.getRange(
                startCol: 1,
                endCol: node.cmarkNode.pointee.start_column - 1,
                startLine: node.cmarkNode.pointee.start_line,
                endLine: node.cmarkNode.pointee.start_line
            )
            
            let startOfLine = self.input.substring(with: itemDefinition)
            newParent = ParagraphAR(indexes, node, itemParent, startOfLine as NSString)
            typeAssist.nodeOfInterest(nodeRange: newParent.range, startOfLine, lineStartRange: itemDefinition)
        } else {
            newParent = ParagraphAR(indexes, node, currentParent!)
        }
        self.currentParent = newParent
        processedDocument.append(newParent)
        let _ = visitChildren(of: node)
        self.currentParent = oldParent
    }

    public func visit(heading node: Heading)  {
        let oldParent = self.currentParent
        var newParent: AttributeRange
        // if you were to create a `-` style list, and then hit enter and then tab, it is valid for that tab
        // to be a heading. This is not likely what the user expects to happen when they create a bulleted list
        // and tab. However most parsers will output this heading according to commonmark spec. This is the one
        // place our "Preview" will not behave the same way a parser would render. We should think about how we
        // feel about that. 
        if let itemParent = oldParent as? ItemAR {
            let itemDefinition = indexes.getRange(
                startCol: 1,
                endCol: node.cmarkNode.pointee.end_column,
                startLine: node.cmarkNode.pointee.start_line,
                endLine: node.cmarkNode.pointee.start_line
            )

            let startOfLine = self.input.substring(with: itemDefinition)
            newParent = ParagraphAR(indexes, node, itemParent, startOfLine as NSString)
            typeAssist.nodeOfInterest(nodeRange: newParent.range, startOfLine, lineStartRange: itemDefinition)
        } else {
            newParent = HeadingAR(indexes, node, currentParent!)
        }
        self.currentParent = newParent
        processedDocument.append(newParent)
        let _ = visitChildren(of: node)
        self.currentParent = oldParent
    }

    public func visit(thematicBreak node: ThematicBreak)  {
        let _ = visitChildren(of: node)
    }

    public func visit(text node: Text)  {
        let oldParent = self.currentParent
        let newParent = TextAR(indexes, node, currentParent!)
        self.currentParent = newParent
        processedDocument.append(newParent)
        let _ = visitChildren(of: node)
        self.currentParent = oldParent
    }

    public func visit(softBreak node: SoftBreak)  {
        //        return report(node)
    }

    public func visit(lineBreak node: LineBreak)  {
        //        return report(node)
    }

    public func visit(code node: Code)  {
        let oldParent = self.currentParent
        let newParent = InlineCodeAR(indexes, node, currentParent!)
        self.currentParent = newParent
        processedDocument.append(newParent)
        let _ = visitChildren(of: node)
        self.currentParent = oldParent
    }

    public func visit(htmlInline node: HtmlInline)  {
        let _ = visitChildren(of: node)
    }

    public func visit(customInline node: CustomInline)  {
        let _ = visitChildren(of: node)
    }

    public func visit(emphasis node: Emphasis)  {
        let oldParent = self.currentParent
        let newParent = EmphasisAR(indexes, node, currentParent!)
        self.currentParent = newParent
        processedDocument.append(newParent)
        let _ = visitChildren(of: node)
        self.currentParent = oldParent
        
    }

    public func visit(strong node: Strong)  {
        let oldParent = self.currentParent
        let newParent = StrongAR(indexes, node, currentParent!)
        self.currentParent = newParent
        processedDocument.append(newParent)
        let _ = visitChildren(of: node)
        self.currentParent = oldParent
    }

    public func visit(link node: Link)  {
        let oldParent = self.currentParent
        let newParent = LinkAR(indexes, node, currentParent!)
        self.currentParent = newParent
        processedDocument.append(newParent)
        let _ = visitChildren(of: node)
        self.currentParent = oldParent
    }

    public func visit(image node: Image)  {
        let _ = visitChildren(of: node)
    }

}
