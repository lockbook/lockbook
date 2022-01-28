#if os(iOS)
import UIKit
#elseif os(macOS)
import AppKit
#endif

public struct Theme {
    /// The background color of the Notepad.
    public var backgroundColor: UniversalColor = UniversalColor.clear
    /// The tint color (AKA cursor color) of the Notepad.
    public var tintColor: UniversalColor = UniversalColor.blue


    public init() {
    }

    /// Sets the background color, tint color, etc. of the Notepad editor.
    ///
    /// - parameter attributes: The attributes to parse for the editor.
    mutating func configureEditor(_ attributes: [String: AnyObject]) {
        if let bgColor = attributes["backgroundColor"] {
            let value = bgColor as! String
            backgroundColor = UniversalColor(hexString: value)
        }

        if let tint = attributes["tintColor"] {
            let value = tint as! String
            tintColor = UniversalColor(hexString: value)
        }
    }
}
