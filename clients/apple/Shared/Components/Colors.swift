import SwiftUI
#if os(macOS)
import AppKit
extension Color {
    static let tint = Color("Tint")
    static let lightBackground = Color("LightBackground")
    static let darkBackground = Color("DarkBackground")
    static let text = Color("Text")

    static let lightText = Color(NSColor.textColor)
    static let darkText = Color(NSColor.textColor)
    static let placeholderText = Color(NSColor.placeholderTextColor)

    static let label = Color(NSColor.labelColor)
    static let secondaryLabel = Color(NSColor.secondaryLabelColor)
    static let tertiaryLabel = Color(NSColor.tertiaryLabelColor)
    static let quaternaryLabel = Color(NSColor.quaternaryLabelColor)
    
    static let systemBackground = Color(NSColor.windowBackgroundColor)
    static let secondarySystemBackground = Color(NSColor.windowBackgroundColor)
    static let tertiarySystemBackground = Color(NSColor.windowBackgroundColor)

    static let systemGray = Color(NSColor.systemGray)
    static let systemGray2 = Color(NSColor.systemGray)
    static let systemGray3 = Color(NSColor.systemGray)
    static let systemGray4 = Color(NSColor.systemGray)
    static let systemGray5 = Color(NSColor.systemGray)
    static let systemGray6 = Color(NSColor.systemGray)

    static let separator = Color(NSColor.separatorColor)
    static let opaqueSeparator = Color(NSColor.separatorColor)
    static let link = Color(NSColor.linkColor)

    static var systemRed: Color { return Color(NSColor.systemRed) }
    static var systemBlue: Color { return Color(NSColor.systemBlue) }
    static var systemPink: Color { return Color(NSColor.systemPink) }
    static var systemTeal: Color { return Color(NSColor.systemTeal) }
    static var systemGreen: Color { return Color(NSColor.systemGreen) }
    static var systemIndigo: Color { return Color(NSColor.systemIndigo) }
    static var systemOrange: Color { return Color(NSColor.systemOrange) }
    static var systemPurple: Color { return Color(NSColor.systemPurple) }
    static var systemYellow: Color { return Color(NSColor.systemYellow) }
}
#else
import UIKit
extension Color {
    static let tint = Color("Tint")
    static let lightBackground = Color("LightBackground")
    static let darkBackground = Color("DarkBackground")
    static let text = Color("Text")

    static let lightText = Color(UIColor.lightText)
    static let darkText = Color(UIColor.darkText)
    static let placeholderText = Color(UIColor.placeholderText)

    static let label = Color(UIColor.label)
    static let secondaryLabel = Color(UIColor.secondaryLabel)
    static let tertiaryLabel = Color(UIColor.tertiaryLabel)
    static let quaternaryLabel = Color(UIColor.quaternaryLabel)

    static let systemBackground = Color(UIColor.systemBackground)
    static let secondarySystemBackground = Color(UIColor.secondarySystemBackground)
    static let tertiarySystemBackground = Color(UIColor.tertiarySystemBackground)
    
    static let systemGray = Color(UIColor.systemGray)
    static let systemGray2 = Color(UIColor.systemGray2)
    static let systemGray3 = Color(UIColor.systemGray3)
    static let systemGray4 = Color(UIColor.systemGray4)
    static let systemGray5 = Color(UIColor.systemGray5)
    static let systemGray6 = Color(UIColor.systemGray6)

    static let separator = Color(UIColor.separator)
    static let opaqueSeparator = Color(UIColor.opaqueSeparator)
    static let link = Color(UIColor.link)

    static var systemRed: Color { return Color(UIColor.systemRed) }
    static var systemBlue: Color { return Color(UIColor.systemBlue) }
    static var systemPink: Color { return Color(UIColor.systemPink) }
    static var systemTeal: Color { return Color(UIColor.systemTeal) }
    static var systemGreen: Color { return Color(UIColor.systemGreen) }
    static var systemIndigo: Color { return Color(UIColor.systemIndigo) }
    static var systemOrange: Color { return Color(UIColor.systemOrange) }
    static var systemPurple: Color { return Color(UIColor.systemPurple) }
    static var systemYellow: Color { return Color(UIColor.systemYellow) }
}
#endif

extension Color {
    static func textEditorBackground(isDark: Bool) -> Color {
        return isDark ? textEditorBackgroundDark : textEditorBackgroundLight
    }
    static let textEditorBackgroundLight = Color(red: 1.0, green: 1.0, blue: 1.0, opacity: 1.0)
    static let textEditorBackgroundDark = Color(red: 30.0/256.0, green: 30.0/256.0, blue: 30.0/256.0, opacity: 1.0)
}
