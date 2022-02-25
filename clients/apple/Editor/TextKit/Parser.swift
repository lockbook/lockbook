import Foundation
import Down

/// This visitor will generate the debug description of an entire abstract syntax tree,
/// indicating relationships between nodes with indentation.

public class Parser: Visitor {

    private var indexes: IndexConverter
    public var processedDocument: [AttributeRange] = []
    private var depth = 0

    public init(_ input: String) {
        self.indexes = IndexConverter(string: input)
        let document = (try? Down(markdownString: input).toDocument())!
        self.visit(document: document)
    }
    
    public func base() -> AttributeRange {
        AttributeRange(
            attribute: Styler.body(),
            range: indexes.wholeDocument()
        )
    }

//    private func report(_ node: Node)  {
//        return "\(indent)\(node is Document ? "" : "↳ ")\(String(reflecting: node)) range: [\(node.cmarkNode.pointee.start_column) - \(node.cmarkNode.pointee.end_column)] \n"
//    }
//
//    private func reportWithChildren(_ node: Node)  {
//        let thisNode = report(node)
//        depth += 1
//        let children = visitChildren(of: node).joined()
//        depth -= 1
//        return "\(thisNode)\(children)"
//    }

    public func visit(document node: Document)  {
        let _ = visitChildren(of: node)
    }

    public func visit(blockQuote node: BlockQuote)  {
//        return reportWithChildren(node)
    }

    public func visit(list node: List)  {
        //        return reportWithChildren(node)
    }

    public func visit(item node: Item)  {
        //        return reportWithChildren(node)
    }

    public func visit(codeBlock node: CodeBlock)  {
        //        return reportWithChildren(node)
    }

    public func visit(htmlBlock node: HtmlBlock)  {
        //        return reportWithChildren(node)
    }

    public func visit(customBlock node: CustomBlock)  {
        //        return reportWithChildren(node)
    }

    public func visit(paragraph node: Paragraph)  {
        //        return reportWithChildren(node)
    }

    public func visit(heading node: Heading)  {
        let range = NSRange(location: 0, length: 3)
        let style = Styler.style(node)
        processedDocument.append(AttributeRange(attribute: style, range: range))
    }

    public func visit(thematicBreak node: ThematicBreak)  {
//        return report(node)
    }

    public func visit(text node: Text)  {
        //        return report(node)
    }

    public func visit(softBreak node: SoftBreak)  {
        //        return report(node)
    }

    public func visit(lineBreak node: LineBreak)  {
        //        return report(node)
    }

    public func visit(code node: Code)  {
        //        return report(node)
    }

    public func visit(htmlInline node: HtmlInline)  {
        //        return report(node)
    }

    public func visit(customInline node: CustomInline)  {
        //        return report(node)
    }

    public func visit(emphasis node: Emphasis)  {
        //        return report(node)
    }

    public func visit(strong node: Strong)  {
        //        return reportWithChildren(node)
    }

    public func visit(link node: Link)  {
        //        return reportWithChildren(node)
    }

    public func visit(image node: Image)  {
        //        return reportWithChildren(node)
    }

}

public struct AttributeRange {
    let attribute: [NSAttributedString.Key : Any]
    let range: NSRange
}

public class IndexConverter {
    
    private let string: String
    
    /// string.count of each line
    private var columnLookup: [Int] = []
    
    init(string: String) {
        self.string = string
        
        let counts = string
            .components(separatedBy: .newlines)
            .map { $0.utf8.count + 1 } // \n
        
        self.columnLookup.reserveCapacity(counts.count)
        
        var sum = 0
        for count in counts {
            sum += count
            self.columnLookup.append(sum)
        }
    }
    
    public func getUTF8Index(utf8Row: Int32, utf8Col: Int32) -> Int {
        var previousLineCount = 0
        if utf8Row != 0 {
            previousLineCount += columnLookup[  Int(utf8Row)]
        }
        
        return previousLineCount + Int(utf8Col)
    }
    
    public func getRange(node: BaseNode) -> NSRange {
        let pointee = node.cmarkNode.pointee
        
        let startUTF8 = getUTF8Index(utf8Row: pointee.start_line, utf8Col: pointee.start_column)
        let offset = getUTF8Index(utf8Row: pointee.end_line, utf8Col: pointee.end_column) - startUTF8
        
        let start = string.utf8.index(string.startIndex, offsetBy: startUTF8)
        let end = string.utf8.index(start, offsetBy: offset)
        
        return NSRange(start...end, in: string)
    }
    
    public func wholeDocument() -> NSRange {
        NSRange(location: 0, length: string.utf16.count)
    }
}
