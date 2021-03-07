public struct Drawing: Codable {
    public let scale: Float
    public let translationX: Float
    public let translationY: Float
    public let strokes: [Stroke]
}

public struct Stroke: Codable {
    public let pointsX: [Float]
    public let pointsY: [Float]
    public let pointsGirth: [Float]
    public let color: ColorAlias
    public let alpha: Float
}

public enum ColorAlias: String, Codable {
    case Black
    case Blue
    case Cyan
    case Green
    case Magenta
    case Red
    case White
    case Yellow
}