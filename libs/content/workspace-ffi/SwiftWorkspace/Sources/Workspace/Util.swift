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
            case Grab:
                NSCursor.openHand
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

    extension NSImage {
        /// Best-effort conversion to PNG bytes for sending through the workspace FFI.
        func lb_pngData() -> Data? {
            if let tiff = tiffRepresentation,
               let bitmap = NSBitmapImageRep(data: tiff)
            {
                return bitmap.representation(using: .png, properties: [:])
            }
            return nil
        }
    }
#endif

#if os(iOS)
    import UIKit

    public extension UIImage {
        /// The camera's native orientation is landscape (with correction in EXIF metadata)
        /// Workspace doesn't handle EXIF data, so this function automatically corrects the orientation
        func normalizedImage() -> UIImage {
            if imageOrientation == .up { return self }

            let format = UIGraphicsImageRendererFormat.default()
            format.scale = scale
            format.opaque = false

            let renderer = UIGraphicsImageRenderer(size: size, format: format)

            return renderer.image { _ in
                self.draw(in: CGRect(origin: .zero, size: self.size))
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
        uuid.0 == 0 &&
            uuid.1 == 0 &&
            uuid.2 == 0 &&
            uuid.3 == 0 &&
            uuid.4 == 0 &&
            uuid.5 == 0 &&
            uuid.6 == 0 &&
            uuid.7 == 0 &&
            uuid.8 == 0 &&
            uuid.9 == 0 &&
            uuid.10 == 0 &&
            uuid.11 == 0 &&
            uuid.12 == 0 &&
            uuid.13 == 0 &&
            uuid.14 == 0 &&
            uuid.15 == 0
    }
}
