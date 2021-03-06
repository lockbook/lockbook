import Foundation

public struct MarkdownNode {
    public let range: NSRange
    public let rawType: Int
    public let type: MarkdownType
    public let headingLevel: Int

    public init?(_ s: (Int, Int), _ e: (Int, Int), _ rawType: Int, lines: [Int], headingLevel: Int = 0) {
        self.rawType = rawType
        if let type = MarkdownType(rawValue: self.rawType) {
            let s = lines[s.0] + s.1
            let e = lines[e.0] + e.1
            self.range = NSMakeRange(s, e - s + 1)
            self.headingLevel = headingLevel
            self.type = type
        } else {
            return nil
        }
    }

    public enum MarkdownType: Int {
        case quote = 2
        case list = 4
        case codeFence = 5
        case header = 9
        case code = 14
        case italic = 17
        case bold = 18
    }
}
