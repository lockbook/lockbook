import SwiftLockbookCore
import PencilKit

var force_multiplier: Float = 2
var force_constant: Float = 2
var force_min: Float = 1.5

extension PKDrawing {
    init(from: Drawing) {
        self.init(
                strokes: from.strokes.map(
                        { lbStroke in PKStroke(from: lbStroke) }
                )
        )
    }
}

extension PKStroke {

    init(from: Stroke) {
        var points = [PKStrokePoint]()
        for index in 0...from.pointsX.count-1 {
            points.append(
                    PKStrokePoint(
                            location: CGPoint(
                                    x: CGFloat(from.pointsX[index]),
                                    y: CGFloat(from.pointsY[index])
                            ),
                            timeOffset: 1,
                            size: CGSize(
                                    width: Double(from.pointsGirth[index] * force_multiplier + force_constant),
                                    height: Double(from.pointsGirth[index] * force_multiplier + force_constant)
                            ),
                            opacity: 1,
                            force: 1,
                            azimuth: 1,
                            altitude: 1
                    )
            )
        }

        self.init(
                ink: PKInk(.pen, color: UIColor(from: from.color)),
                path: PKStrokePath(controlPoints: points, creationDate: Date())
        )
    }
}

extension UIColor {
    convenience init(from: ColorAlias) {
        switch from {
        case .Black: self.init(.black)
        case .Blue: self.init(.blue)
        case .Cyan: self.init(red: CGFloat(0.188235294), green: CGFloat(0.835294118), blue: CGFloat(0.784313725), alpha: CGFloat(1))
        case .Green: self.init(.green)
        case .Magenta: self.init(red: CGFloat(1), green: CGFloat(0), blue: CGFloat(1), alpha: CGFloat(1))
        case .Red: self.init(.red)
        case .White: self.init(.white)
        case .Yellow: self.init(.yellow)
        }
    }
}

