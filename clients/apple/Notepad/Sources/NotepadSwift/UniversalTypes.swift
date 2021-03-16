#if os(iOS)
import UIKit
public typealias UniversalColor = UIColor
public typealias UniversalFont = UIFont
public typealias UniversalFontDescriptor = UIFontDescriptor
public typealias UniversalTraits = UIFontDescriptor.SymbolicTraits
#elseif os(macOS)
import AppKit
public typealias UniversalColor = NSColor
public typealias UniversalFont = NSFont
public typealias UniversalFontDescriptor = NSFontDescriptor
public typealias UniversalTraits = NSFontDescriptor.SymbolicTraits

extension UniversalColor {
    public static let label = labelColor
    public static let secondaryLabel = secondaryLabelColor
    public static let tertiaryLabel = tertiaryLabelColor
    public static let quaternaryLabel = quaternaryLabelColor
    public static let link = linkColor
    public static let placeholderText = placeholderTextColor
    public static let windowFrameText = windowFrameTextColor
    public static let selectedMenuItemText = selectedMenuItemTextColor
    public static let alternateSelectedControlText = alternateSelectedControlTextColor
    public static let headerText = headerTextColor
    public static let separator = separatorColor
    public static let grid = gridColor
    public static let windowBackground = windowBackgroundColor
}
#endif
