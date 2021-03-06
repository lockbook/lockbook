import Foundation
import NotepadSwift
import Down

public class MarkdownEngine {
    var lines: [Int] = []

    public init() {

    }

    private func toMarkdownNode(_ node: Node) -> MarkdownNode? {
        let p = node.cmarkNode.pointee
        return MarkdownNode(
            (Int(p.start_line) - 1, Int(p.start_column) - 1),
            (Int(p.end_line) - 1, Int(p.end_column) - 1),
            Int(p.type),
            lines: lines,
            headingLevel: node.cmarkNode.headingLevel
        )
    }

    func exploreChildren(_ node: Node) -> [MarkdownNode] {
        node.children.reduce(toMarkdownNode(node).map { c in [c] } ?? []) { (r, c) in
            r + exploreChildren(c)
        }
    }

    public func render(_ markdownString: String) -> [MarkdownNode] {
        let lcs = markdownString.components(separatedBy: .newlines).map { $0.count }
        var sum = 0
        var counts: [Int] = [0]
        for l in lcs {
            sum += (l + 1)
            counts.append(sum)
        }
        lines = counts

        let result = (try? Down(markdownString: markdownString).toAST(.sourcePos))!
        let p = result.wrap()!
        
        return exploreChildren(p)
    }
}
