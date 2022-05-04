import Foundation
import Down

public class TypeAssist {
    
    let indexer: IndexConverter
    var assistableNodes: [AssistableNode] = []
    
    init(_ indexer: IndexConverter) {
        self.indexer = indexer
    }
    
    public func mightAssist(_ str: String) -> Bool {
        (str == "\n" || str == "\t") && !assistableNodes.isEmpty
    }
    
    public func nodeOfInterest(nodeRange: NSRange, _ lineStart: String, lineStartRange: NSRange, fresh: Bool = false) {
        let node = AssistableNode(fresh: fresh, startOfLine: lineStartRange, lineHeader: lineStart, nodeRange: nodeRange)
        assistableNodes.append(node)
    }
    
    public func assist(_ string: String, _ replacementRange: NSRange) -> (String, NSRange) {
        for node in assistableNodes {
            if string == "\n"
                && replacementRange.length == 0
                && node.nodeRange.contains(replacementRange.location - 1) {
                if node.fresh {
                    return ("\n", node.startOfLine)
                } else {
                    return (node.newLine(), replacementRange)
                }
            }
            
            if string == "\t" && node.nodeRange.contains(replacementRange.location - 1) {
                return node.indent()
            }
        }
        
        return (string, replacementRange)
    }
}

struct AssistableNode {
    var fresh: Bool
    var startOfLine: NSRange
    var lineHeader: String
    var nodeRange: NSRange
    
    func newLine() -> String {
        return "\n" + next(lineHeader)
    }
    
    private func next(_ current: String) -> String {
        let maybeNumber = current.filter("0123456789".contains)
        if maybeNumber.isEmpty {
            return current
        } else {
            if let oldNumber = Int(maybeNumber) {
                let nonWhiteSpaceOffset = current.count - current.trimmingCharacters(in: .whitespaces).count
                if nonWhiteSpaceOffset > 0 {
                    let index = current.index(current.startIndex, offsetBy: nonWhiteSpaceOffset - 1)
                    let whitespace = current[..<index]
                    return whitespace + String(oldNumber + 1) + ". "
                } else {
                    // TODO handle ) here in the future
                    return String(oldNumber + 1) + ". "
                }
                
            } else {
                return current
            }
        }
    }
    
    func indent() -> (String, NSRange) {
        return ("\t", NSRange(location: startOfLine.location, length: 0))
    }
}

