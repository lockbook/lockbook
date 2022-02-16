import Foundation
#if os(iOS)
import UIKit
#elseif os(macOS)
import AppKit
#endif

public class Storage: NSTextStorage {
    var backingStore = NSMutableAttributedString()
    
    public override var string: String {
        get {
            backingStore.string
        }
    }
    
    public override func attributes(at location: Int, effectiveRange range: NSRangePointer?) -> [NSAttributedString.Key : Any] {
        backingStore.attributes(at: location, effectiveRange: range)
    }
    
    public override func replaceCharacters(in range: NSRange, with str: String) {
        backingStore.replaceCharacters(in: range, with: str)
        self.edited([.editedCharacters, .editedAttributes], range: range, changeInLength: (str as NSString).length - range.length)
    }
    
    public override func setAttributes(_ attrs: [NSAttributedString.Key : Any]?, range: NSRange) {
        backingStore.setAttributes(attrs, range: range)
        self.edited(.editedAttributes, range: range, changeInLength: 0)
    }
}
