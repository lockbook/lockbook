import Foundation
import Down
import AppKit

class Styler {
    
    static func body() -> [NSAttributedString.Key : Any] {
        [
            .foregroundColor : UniversalColor.labelColor,
            .font :  NSFont.systemFont(ofSize: NSFont.systemFontSize),
        ]
    }
    
    static func style(_ heading: Heading) -> [NSAttributedString.Key : Any] {
        [
            .foregroundColor : UniversalColor.fromColorAlias(from: .Red).blendColors(UniversalColor.fromColorAlias(from: .Red), by: (CGFloat(heading.headingLevel-1)/10)),
            .font : NSFont(descriptor: NSFont.systemFont(ofSize: NSFont.systemFontSize).fontDescriptor.withSymbolicTraits([.bold, .expanded]), size: NSFont.systemFontSize)!,
        ]
    }
}
