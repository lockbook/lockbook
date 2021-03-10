public struct Drawing: Codable {
    public let scale: Float
    public let translationX: Float
    public let translationY: Float
    public let strokes: [Stroke]

    public init() {
        scale = 1.0
        translationX = 1.0
        translationY = 1.0
        strokes = []
    }

    public init(scale: Float, translationX: Float, translationY: Float, strokes: [Stroke]) {
        self.scale = scale
        self.translationX = translationX
        self.translationY = translationY
        self.strokes = strokes
    }
}

public struct Stroke: Codable {
    public let pointsX: [Float]
    public let pointsY: [Float]
    public let pointsGirth: [Float]
    public let color: ColorAlias
    public let alpha: Float

    public init(pointsX: [Float], pointsY: [Float], pointsGirth: [Float], color: ColorAlias, alpha: Float) {
        self.pointsX = pointsX
        self.pointsY = pointsY
        self.pointsGirth = pointsGirth
        self.color = color
        self.alpha = alpha
    }
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