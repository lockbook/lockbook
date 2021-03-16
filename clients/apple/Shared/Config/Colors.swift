import SwiftUI
import SwiftLockbookCore
import NotepadSwift

extension Color {
    static func textEditorBackground(isDark: Bool) -> Color {
        return isDark ? textEditorBackgroundDark : textEditorBackgroundLight
    }
    static let textEditorBackgroundLight = Color(red: 1.0, green: 1.0, blue: 1.0, opacity: 1.0)
    static let textEditorBackgroundDark = Color(red: 30.0/256.0, green: 30.0/256.0, blue: 30.0/256.0, opacity: 1.0)
}

extension ColorAlias {
    static func fromUIColor(from color: UniversalColor) -> ColorAlias {
        switch color {
        case .fromColorAlias(from: .Black): return .Black
        case .fromColorAlias(from: .White): return .White
        case .fromColorAlias(from: .Cyan): return .Cyan
        case .fromColorAlias(from: .Magenta): return .Magenta
        case .fromColorAlias(from: .Red): return .Red
        case .fromColorAlias(from: .Green): return .Green
        case .fromColorAlias(from: .Blue): return .Blue
        case .fromColorAlias(from: .Yellow): return .Yellow
        default: return .Black
        }
    }
}

extension UniversalColor {
    static func fromColorAlias(from color: ColorAlias) -> UniversalColor {
        switch color {
        case .Black: return .black
        case .Blue: return .systemBlue
        case .Cyan: return .systemTeal
        case .Yellow: return .systemYellow
        case .Magenta: return .systemPurple
        case .Red: return .systemPink
        case .White: return .white
        case .Green: return .systemGreen
        }
    }
}
