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
        let x: UniversalColor = {
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
        } ()
        #if os(iOS)
        return x.resolvedColor(with: .current)
        #else
        return x
        #endif
    }

    #if os(iOS)
    func addColor(_ color1: UniversalColor, with color2: UniversalColor) -> UniversalColor {
        var (r1, g1, b1, a1) = (CGFloat(0), CGFloat(0), CGFloat(0), CGFloat(0))
        var (r2, g2, b2, a2) = (CGFloat(0), CGFloat(0), CGFloat(0), CGFloat(0))
        color1.getRed(&r1, green: &g1, blue: &b1, alpha: &a1)
        color2.getRed(&r2, green: &g2, blue: &b2, alpha: &a2)
        return UniversalColor(red: min(r1 + r2, 1), green: min(g1 + g2, 1), blue: min(b1 + b2, 1), alpha: (a1 + a2) / 2)
    }
    func multiplyColor(_ color: UniversalColor, by multiplier: CGFloat) -> UniversalColor {
        var (r, g, b, a) = (CGFloat(0), CGFloat(0), CGFloat(0), CGFloat(0))
        color.getRed(&r, green: &g, blue: &b, alpha: &a)
        return UniversalColor(red: r * multiplier, green: g * multiplier, blue: b * multiplier, alpha: a)
    }

    func blendColors(_ color: UniversalColor, by multiplier: CGFloat) -> UniversalColor {
        let base = multiplyColor(self, by: 1 - multiplier)
        let other = multiplyColor(color, by: multiplier)
        return addColor(base, with: other)
    }
    #else
    func blendColors(_ color: UniversalColor, by multiplier: CGFloat) -> UniversalColor {
        return self.blended(withFraction: multiplier, of: color)!
    }
    #endif

}
