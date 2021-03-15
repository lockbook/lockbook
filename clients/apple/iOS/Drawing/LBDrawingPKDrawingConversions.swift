import SwiftLockbookCore
import NotepadSwift
import PencilKit

var force_multiplier: Float = 0.5
var force_constant: Float = 1.1 + 0.6
var interpolationDistance: Float = 0.5

extension PKDrawing {
    init(from: Drawing) {
        self.init(
                strokes: from.strokes.map(
                        { lbStroke in PKStroke(from: lbStroke) }
                )
        )
    }
}

extension Drawing {
    init(from: PKDrawing) {

        let strokes = from.strokes.map { Stroke(from: $0) }

        self.init(scale: 1, translationX: 0, translationY: 0, strokes: strokes)
    }
}

extension Stroke {
    init(from: PKStroke) {

        let color = ColorAlias.fromUIColor(from: from.ink.color)
        var pointsX = [Float]()
        var pointsY = [Float]()
        var pointsGirth = [Float]()

        for point in from.path.interpolatedPoints(by: .distance(CGFloat(interpolationDistance))) {
            pointsX.append(Float(point.location.x.native + from.transform.tx.native))
            pointsY.append(Float(point.location.y.native + from.transform.ty.native))
            pointsGirth.append((Float(point.size.width.native) - force_constant) / force_multiplier)
        }

        self.init(pointsX: pointsX, pointsY: pointsY, pointsGirth: pointsGirth, color: color, alpha: Float(1.0)) // TODO alpha
    }
}

extension PKStroke {

    init(from: Stroke) {
        var points = [PKStrokePoint]()
        for index in 0...from.pointsX.count - 1 {
            let point = PKStrokePoint(
                    location: CGPoint(
                            x: CGFloat(from.pointsX[index]),
                            y: CGFloat(from.pointsY[index])
                    ),
                    timeOffset: 1,
                    size: CGSize(
                            width: Double(from.pointsGirth[index] * force_multiplier + force_constant),
                            height: Double(from.pointsGirth[index] * force_multiplier + force_constant)
                    ),
                    opacity: 1, // TODO alpha
                    force: 1,
                    azimuth: 1,
                    altitude: 1
            )

            points.append(point)
            points.append(point)
            points.append(point)
            points.append(point)
            points.append(point)
            points.append(point)
        }

        self.init(
                ink: PKInk(.pen, color: .fromColorAlias(from: from.color)), // TODO .pencil highlighter
                path: PKStrokePath(controlPoints: points, creationDate: Date())
        )
    }
}
