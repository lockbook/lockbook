import Foundation
import Down

/// This visitor will generate the debug description of an entire abstract syntax tree,
/// indicating relationships between nodes with indentation.

public class Parser: Visitor {
    
    public var indexes: IndexConverter
    public var processedDocument: [AttributeRange] = []
    private var depth = 0
    
    public init(_ input: String) {
        self.indexes = IndexConverter(input)
        let document = (try? Down(markdownString: input).toDocument())!
        self.visit(document: document)
    }
    
    public func base() -> AttributeRange {
        AttributeRange(
            style: .Base,
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
        processedDocument.append(
            AttributeRange(
                style: Style.from(node),
                range: indexes.getRange(node)
            )
        )
        let _ = visitChildren(of: node)
    }
    
    public func visit(blockQuote node: BlockQuote)  {
        let _ = visitChildren(of: node)
    }
    
    public func visit(list node: List)  {
        let _ = visitChildren(of: node)
    }
    
    public func visit(item node: Item)  {
        let _ = visitChildren(of: node)
    }
    
    public func visit(codeBlock node: CodeBlock)  {
        let range = indexes.getRange(node)
        let style = Style.from(node)
        processedDocument.append(AttributeRange(style: style, range: range))
        let _ = visitChildren(of: node)
    }
    
    public func visit(htmlBlock node: HtmlBlock)  {
        let _ = visitChildren(of: node)
    }
    
    public func visit(customBlock node: CustomBlock)  {
        let _ = visitChildren(of: node)
    }
    
    public func visit(paragraph node: Paragraph)  {
        let _ = visitChildren(of: node)
    }
    
    public func visit(heading node: Heading)  {
        let range = indexes.getRange(node)
        let style = Style.from(node)
        processedDocument.append(AttributeRange(style: style, range: range))
        let _ = visitChildren(of: node)
    }
    
    public func visit(thematicBreak node: ThematicBreak)  {
        visitChildren(of: node)
    }
    
    public func visit(text node: Text)  {
        print("Text start line: \(node.cmarkNode.pointee.start_line), endline: \(node.cmarkNode.pointee.end_line), start_column: \(node.cmarkNode.pointee.start_column), end_column: \(node.cmarkNode.pointee.end_column)")
    }
    
    public func visit(softBreak node: SoftBreak)  {
        //        return report(node)
    }
    
    public func visit(lineBreak node: LineBreak)  {
        //        return report(node)
    }
    
    public func visit(code node: Code)  {
        print("Code start line: \(node.cmarkNode.pointee.start_line), endline: \(node.cmarkNode.pointee.end_line), start_column: \(node.cmarkNode.pointee.start_column), end_column: \(node.cmarkNode.pointee.end_column)")
        let range = indexes.getRange(node)
        let style = Style.from(node)
        processedDocument.append(AttributeRange(style: style, range: range))
        let _ = visitChildren(of: node)
    }
    
    public func visit(htmlInline node: HtmlInline)  {
        let _ = visitChildren(of: node)
    }
    
    public func visit(customInline node: CustomInline)  {
        let _ = visitChildren(of: node)
    }
    
    public func visit(emphasis node: Emphasis)  {
        print("Text start line: \(node.cmarkNode.pointee.start_line), endline: \(node.cmarkNode.pointee.end_line), start_column: \(node.cmarkNode.pointee.start_column), end_column: \(node.cmarkNode.pointee.end_column)")
        let _ = visitChildren(of: node)
    }
    
    public func visit(strong node: Strong)  {
        let _ = visitChildren(of: node)
    }
    
    public func visit(link node: Link)  {
        let range = indexes.getRange(node)
        let style = Style.from(node)
        processedDocument.append(AttributeRange(style: style, range: range))
        let _ = visitChildren(of: node)
    }
    
    public func visit(image node: Image)  {
        let _ = visitChildren(of: node)
    }
    
}

public struct AttributeRange {
    let style: Style
    let range: NSRange
}

public class IndexConverter {
    
    private let string: String
    
    /// string.count of each line
    public var columnLookup: [Int] = []
    
    init(_ string: String) {
        self.string = string
        print("size: \((string as NSString).length)")

        let counts = string
            .components(separatedBy: .newlines)
            .map { $0.utf8.count }
        
        self.columnLookup.reserveCapacity(counts.count)
        
        var sum = 0
        for count in counts {
            sum += count
            self.columnLookup.append(sum)
        }
        print(columnLookup)
    }
    
    public func getUTF8Index(utf8Row: Int32, utf8Col: Int32) -> Int {
        var previousLineCount = 0
        if utf8Row >= 1 {
            let previousLineIndex = Int(utf8Row - 1)
            previousLineCount += columnLookup[previousLineIndex]
            previousLineCount += Int(utf8Row) // How many newline chars until this point
        }
        
        return previousLineCount + Int(utf8Col)
    }
    
    public func getRange(_ node: BaseNode) -> NSRange {
        let pointee = node.cmarkNode.pointee
        
        return getRange(
            startCol: pointee.start_column,
            endCol: pointee.end_column,
            startLine: pointee.start_line,
            endLine: pointee.end_line
        )
    }
    
    public func getRange(startCol: Int32, endCol: Int32, startLine: Int32, endLine: Int32) -> NSRange {
        if string.isEmpty && startCol == 1 && endCol == 0 && startLine == 1 && endLine == 0 {
            return NSRange(location: 0, length: 0)
        }
        let startUTF8 = getUTF8Index(utf8Row: startLine-1, utf8Col: startCol-1)
        let offset = getUTF8Index(utf8Row: endLine-1, utf8Col: endCol-1) - startUTF8
        
        let start = string.utf8.index(string.startIndex, offsetBy: startUTF8)
        let end = string.utf8.index(start, offsetBy: offset)
        
            return NSRange(start...end, in: string)
    }
    
    public func wholeDocument() -> NSRange {
        NSRange(location: 0, length: string.utf16.count)
    }
}
