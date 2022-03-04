import Foundation
import Down
import AppKit

enum Style {
    case Base
    case Heading1
}

extension Style {
    static func from(_ heading: Heading) -> Style {
        Style.Heading1
    }
    
    static func from(_ document: Document) -> Style {
        Style.Base
    }
    
    func attributes() -> [NSAttributedString.Key : Any] {
        Styler.style(self)
     }
}

class Styler {
    
    static func body() -> [NSAttributedString.Key : Any] {
        [
            .foregroundColor : NSColor.labelColor,
            .font :  NSFont.systemFont(ofSize: 12),
        ]
    }
    
    static func heading1() -> [NSAttributedString.Key : Any] {
        [
            .foregroundColor : NSColor.labelColor,
            .font : NSFont(descriptor: NSFont.systemFont(ofSize: 24).fontDescriptor.withSymbolicTraits([.bold, .expanded]), size: 24)!,
        ]
    }
    
    static func style(_ style: Style) -> [NSAttributedString.Key : Any] {
        switch style {
        case .Base:
            return body()
        case .Heading1:
            return heading1()
        }
    }
}
