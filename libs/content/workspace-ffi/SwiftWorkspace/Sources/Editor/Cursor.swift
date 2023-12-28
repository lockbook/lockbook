import AppKit
import Bridge

extension NSCursor {
    static func fromCCursor(c: CCursorIcon) -> NSCursor {
        switch c {
        case Text:
            NSCursor.iBeam
        case PointingHand:
            NSCursor.pointingHand
        case Grabbing:
            NSCursor.closedHand
        case Crosshair:
            NSCursor.crosshair
        case Default:
            NSCursor.arrow
        default:
            NSCursor.arrow
        }
    }
}
