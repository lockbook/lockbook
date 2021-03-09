import SwiftLockbookCore
import PencilKit

var force_multiplier: Float = 1
var force_constant: Float = 0
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

        let color = ColorAlias.Black.fromColor(color: from.ink.color)
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
                ink: PKInk(.pen, color: UIColor(from: from.color)), // TODO .pencil highlighter
                path: PKStrokePath(controlPoints: points, creationDate: Date())
        )
    }
}

extension ColorAlias {
    func fromColor(color: UIColor) -> ColorAlias {

        switch color {
        case UIColor(from: .Black): return ColorAlias.Black
        case UIColor(from: .White): return ColorAlias.White
        case UIColor(from: .Cyan): return ColorAlias.Cyan
        case UIColor(from: .Magenta): return ColorAlias.Magenta
        case UIColor(from: .Red): return ColorAlias.Red
        case UIColor(from: .Green): return ColorAlias.Green
        case UIColor(from: .Blue): return ColorAlias.Blue
        case UIColor(from: .Yellow): return ColorAlias.Yellow
        default: return ColorAlias.Black
        }
    }
}

extension UIColor {
    convenience init(from: ColorAlias) {
        switch from {
        case .Black: self.init(red: 0, green: 0, blue: 0, alpha: 1)
        case .Blue: self.init(red: 0.082352941176470587, green: 0.49411764705882355, blue: 0.98431372549019602, alpha: 1)
        case .Cyan: self.init(red: 0.188235294, green: 0.835294118, blue: 0.784313725, alpha: 1)
        case .Yellow: self.init(red: 0.99607843137254903, green: 0.81568627450980391, blue: 0.18823529411764706, alpha: 1)
        case .Magenta: self.init(red: 1, green: 0, blue: 1, alpha: 1)
        case .Red: self.init(red: 0.9882352941176471, green: 0.19215686274509805, blue: 0.25882352941176473, alpha: 1)
        case .White: self.init(red: 1, green: 1, blue: 1, alpha: 1)
        case .Green: self.init(red: 0.32549019607843138, green: 0.84313725490196079, blue: 0.41176470588235292, alpha: 1)
        }
    }
}

