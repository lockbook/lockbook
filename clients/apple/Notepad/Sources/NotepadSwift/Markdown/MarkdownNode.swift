import Foundation
import Down

public struct MarkdownNode: Equatable {
    public let range: NSRange
    public let rawType: Int
    public let type: MarkdownType
    public let headingLevel: Int

    public init(range: NSRange, type: MarkdownType, headingLevel: Int = 0) {
        self.range = range
        self.rawType = type.rawValue
        self.type = type
        self.headingLevel = headingLevel
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
