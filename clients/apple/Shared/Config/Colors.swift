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

        if UniversalColor.fromColorAlias(from: .Black) == color {
            return .Black
        }
        
        if UniversalColor.fromColorAlias(from: .White) == color {
            return .White
        }
        
        if UniversalColor.fromColorAlias(from: .Cyan) == color {
            return .Cyan
        }
        
        if UniversalColor.fromColorAlias(from: .Magenta) == color {
            return .Magenta
        }
        
        if UniversalColor.fromColorAlias(from: .Red) == color {
            return .Red
        }
        
        if UniversalColor.fromColorAlias(from: .Green) == color {
            return .Green
        }
        
        if UniversalColor.fromColorAlias(from: .Blue) == color {
            return .Blue
        }
        
        if UniversalColor.fromColorAlias(from: .Yellow) == color {
            return .Yellow
        }

        print("🚨 unknown color: \(color), we will lose this color and it will become Yellow!")
        return .Yellow
    }
}

#if os(iOS)
public extension UIColor {

    static func == (l: UIColor, r: UIColor) -> Bool {
        var l_red = CGFloat(0); var l_green = CGFloat(0); var l_blue = CGFloat(0); var l_alpha = CGFloat(0)
        guard l.getRed(&l_red, green: &l_green, blue: &l_blue, alpha: &l_alpha) else { return false }
        
        l_red = round(l_red * 100) / 100.0
        l_green = round(l_green * 100) / 100.0
        l_blue = round(l_blue * 100) / 100.0
        
        var r_red = CGFloat(0); var r_green = CGFloat(0); var r_blue = CGFloat(0); var r_alpha = CGFloat(0)
        guard r.getRed(&r_red, green: &r_green, blue: &r_blue, alpha: &r_alpha) else { return false }
        
        r_red = round(r_red * 100) / 100.0
        r_green = round(r_green * 100) / 100.0
        r_blue = round(r_blue * 100) / 100.0

        return l_red == r_red && l_green == r_green && l_blue == r_blue && l_alpha == r_alpha
    }
}
#endif

extension UniversalColor {
    static func fromColorAlias(from color: ColorAlias) -> UniversalColor {
        #if os(iOS)
        switch color {
        case .Black: return UIColor(red: 0/255.0, green: 0/255.0, blue: 0/255.0, alpha: 1)
        case .Blue: return UIColor(red: 0/255.0, green: 122/255.0, blue: 255/255.0, alpha: 1)
        case .Cyan: return UIColor(red: 89/255.0, green: 173/255.0, blue: 196/255.0, alpha: 1)
        case .Yellow: return UIColor(red: 255/255.0, green: 204/255.0, blue: 0/255.0, alpha: 1)
        case .Magenta: return UIColor(red: 175/255.0, green: 82/255.0, blue: 222/255.0, alpha: 1)
        case .Red: return UIColor(red: 255/255.0, green: 45/255.0, blue: 85/255.0, alpha: 1)
        case .White: return UIColor(red: 255/255.0, green: 255/255.0, blue: 255/255.0, alpha: 1)
        case .Green: return UIColor(red: 40/255.0, green: 205/255.0, blue: 65/255.0, alpha: 1)
        }
        #else
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
