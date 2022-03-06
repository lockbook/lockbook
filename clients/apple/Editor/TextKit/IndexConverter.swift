import Foundation
import Down

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
    
    public func getRange(_ node: Node) -> NSRange {
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
