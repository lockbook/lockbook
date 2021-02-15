public struct Drawing: Codable {
    let currentView: Page
    let events: [Event]
}

public struct Event: Codable {
    let stroke: Stroke
}

public struct Stroke: Codable {
    let color: Int
    let points: [Float]
}

public struct Page: Codable {
    var transformation: Transformation
}

public struct Transformation: Codable {
    var translation: Point
    var scale: Float
}

public struct Point: Codable {
    var x: Float
    var y: Float
}
