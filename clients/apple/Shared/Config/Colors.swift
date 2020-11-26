import SwiftUI

extension Color {
    static func textEditorBackground(isDark: Bool) -> Color {
        return isDark ? textEditorBackgroundDark : textEditorBackgroundLight
    }
    static let textEditorBackgroundLight = Color(red: 1.0, green: 1.0, blue: 1.0, opacity: 1.0)
    static let textEditorBackgroundDark = Color(red: 30.0/256.0, green: 30.0/256.0, blue: 30.0/256.0, opacity: 1.0)
}
