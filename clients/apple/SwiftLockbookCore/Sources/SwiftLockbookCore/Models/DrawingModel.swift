import PencilKit

var force_multiplier: Float = 1
var force_constant: Float = 1
var force_min: Float = 1.5

public struct Drawing: Codable {
    public let currentView: Page
    public let events: [Event]
    
    @available(iOS 14.0, *)
    public func getPKDrawing() -> PKDrawing {
        PKDrawing(
            strokes: events.map({stroke in
                PKStroke(
                    ink: PKInk(.pen, color: stroke.stroke.getUIColor()),
                    path: PKStrokePath(
                        controlPoints: stroke.stroke.getPoint().map({point in
                            PKStrokePoint(
                                location: CGPoint(x: CGFloat(point.x), y: CGFloat(point.y)),
                                timeOffset: 1,
                                size: CGSize(width: Double(point.force * force_multiplier + force_constant), height: Double(point.force * force_multiplier + force_constant)),
                                opacity: 1,
                                force: 1,
                                azimuth: 1,
                                altitude: 1
                            )
                        }),
                        creationDate: Date()
                    )
                )
            })
        )
    }
}

public struct Event: Codable {
    public let stroke: Stroke
}

public struct Stroke: Codable {
    public let color: Int
    public let points: [Float]
    
    public func getPoint() -> [StrokePoint] {
        var array = [StrokePoint]()
        
        for index in 0...((points.count/3)-1) {
            
            let force = max(points[index * 3 + 0], force_min)
            let x =     points[index * 3 + 1]
            let y =     points[index * 3 + 2]
            
            print(force)
            
            // TODO a hack to be replaced with: 
            array.append(StrokePoint(force: force, x: x, y: y))
            array.append(StrokePoint(force: force, x: x, y: y))
        }
        
        
        return array
    }
    
    public func getUIColor() -> UIColor {
        
        let modelColor = UIColor(
            red: CGFloat((color >> 16) & 0xFF) / 255.0,
            green: CGFloat((color >> 8) & 0xFF) / 255.0,
            blue: CGFloat(color & 0xFF) / 255.0,
            alpha: CGFloat((color >> 24) & 0xFF)
        )
        
        if color == -1 {
            return .black
        }
        
        return modelColor
    }
}

// Just a helper for getPoint()
public struct StrokePoint {
    public var force: Float
    public var x: Float
    public var y: Float
}

public struct Page: Codable {
    public var transformation: Transformation
}

public struct Transformation: Codable {
    public var translation: Point
    public var scale: Float
}

public struct Point: Codable {
    public var x: Float
    public var y: Float
}
