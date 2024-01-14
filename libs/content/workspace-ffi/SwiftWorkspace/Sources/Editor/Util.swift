import Bridge
import Foundation

#if os(macOS)
import AppKit

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
#endif

func textFromPtr(s: UnsafeMutablePointer<CChar>!) -> String {
    if s == nil {
        return ""
    }
    let str = String(cString: s)
    free_text(s)
    return str
}

extension UUID {
    func isNil() -> Bool {
        self.uuid.0 == 0 &&
        self.uuid.1 == 0 &&
        self.uuid.2 == 0 &&
        self.uuid.3 == 0 &&
        self.uuid.4 == 0 &&
        self.uuid.5 == 0 &&
        self.uuid.6 == 0 &&
        self.uuid.7 == 0 &&
        self.uuid.8 == 0 &&
        self.uuid.9 == 0 &&
        self.uuid.10 == 0 &&
        self.uuid.11 == 0 &&
        self.uuid.12 == 0 &&
        self.uuid.13 == 0 &&
        self.uuid.14 == 0 &&
        self.uuid.15 == 0
    }
}
