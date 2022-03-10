import Foundation
import SwiftUI
import SwiftLockbookCore

#if os(iOS)
import UIKit
public typealias UniversalColor = UIColor
#elseif os(macOS)
import AppKit
public typealias UniversalColor = NSColor
#endif

#if os(macOS)
extension NSColor {
    static let label = NSColor.labelColor
    static let secondaryLabel = NSColor.secondaryLabelColor
}
#endif

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

extension UniversalColor {
    /// Converts a hex color code to UIColor.
    /// http://stackoverflow.com/a/33397427/6669540
    ///
    /// - parameter hexString: The hex code.
    convenience init(hexString: String) {
        let hex = hexString.trimmingCharacters(in: CharacterSet.alphanumerics.inverted)
        var int = UInt64()
        Scanner(string: hex).scanHexInt64(&int)
        let a, r, g, b: UInt64
        switch hex.count {
        case 3: // RGB (12-bit)
            (a, r, g, b) = (255, (int >> 8) * 17, (int >> 4 & 0xF) * 17, (int & 0xF) * 17)
        case 6: // RGB (24-bit)
            (a, r, g, b) = (255, int >> 16, int >> 8 & 0xFF, int & 0xFF)
        case 8: // ARGB (32-bit)
            (a, r, g, b) = (int >> 24, int >> 16 & 0xFF, int >> 8 & 0xFF, int & 0xFF)
        default:
            (a, r, g, b) = (255, 0, 0, 0)
        }
        self.init(red: CGFloat(r) / 255, green: CGFloat(g) / 255, blue: CGFloat(b) / 255, alpha: CGFloat(a) / 255)
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
