import Foundation
import Down

public class MarkdownEngine {
    var attr = NSMutableAttributedString()
    var lines: [Int] = []

    public init() {

    }

    func setAttributes(_ node: Node) {
        let c = CMN(node)
        if (c.type != .other) {
            let s = lines[c.s_l] + c.s_c
            let e = lines[c.e_l] + c.e_c
            let range = NSMakeRange(s, e - s + 1)

            switch c.type {
            case .other:
                ()
            case .header:
                attr.addAttributes([
                    NSAttributedString.Key.font : systemFontWithTraits(headingTraits, fontSize*(10.0-CGFloat(c.headingLevel))/3),
                    NSAttributedString.Key.foregroundColor : headingColor
                ], range: range)
            case .italic:
                attr.addAttributes([
                    NSAttributedString.Key.font : systemFontWithTraits(emphasisTraits)
                ], range: range)
            case .bold:
                attr.addAttributes([
                    NSAttributedString.Key.font : systemFontWithTraits(boldTraits)
                ], range: range)
            case .codeFence:
                attr.addAttributes([
                    NSAttributedString.Key.font : codeFont,
                ], range: range)
            case .list:
                attr.addAttributes([
                    NSAttributedString.Key.foregroundColor : lighterColor
                ], range: range)
            case .quote:
                attr.addAttributes([
                    NSAttributedString.Key.foregroundColor : lighterColor
                ], range: range)
            }
        }
    }

    func exploreChildren(_ node: Node) {
        for c in node.children {
            setAttributes(c)
            exploreChildren(c)
        }
    }

    public func render(_ markdownString: String) -> NSAttributedString {
        self.attr = NSMutableAttributedString(string: markdownString)
        attr.setAttributes([
            NSAttributedString.Key.font : systemFont,
            NSAttributedString.Key.foregroundColor : textColor
        ], range: .init(location: 0, length: markdownString.count))

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
        
        exploreChildren(p)

        return attr
    }
}

struct CMN {
    let s_l: Int
    let s_c: Int
    let e_l: Int
    let e_c: Int
    let rawType: Int
    let type: MDType
    let headingLevel: Int

    init(_ node: Node) {
        let p = node.cmarkNode.pointee
        s_l = Int(p.start_line) - 1
        s_c = Int(p.start_column) - 1
        e_l = Int(p.end_line) - 1
        e_c = Int(p.end_column) - 1
        rawType = Int(p.type)
        type = MDType.init(rawValue: rawType) ?? MDType.other
        headingLevel = node.cmarkNode.headingLevel
    }

    enum MDType: Int {
        case quote = 2
        case list = 4
        case codeFence = 5
        case header = 9
        case italic = 17
        case bold = 18
        case other = 99
    }
}
