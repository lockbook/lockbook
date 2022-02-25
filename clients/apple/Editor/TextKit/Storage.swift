import Foundation
#if os(iOS)
import UIKit
#elseif os(macOS)
import AppKit
#endif
import Down

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
    
    public override func replaceCharacters(in range: NSRange, with string: String) {
        backingStore.replaceCharacters(in: range, with: string)
        self.edited([.editedCharacters], range: range, changeInLength: (string as NSString).length - range.length)
    }
    
    public override func processEditing() {
        let parsed = Parser(backingStore.string)
        let base = parsed.base()
        backingStore.setAttributes(base.attribute, range: base.range)
        for modification in parsed.processedDocument {
            backingStore.addAttributes(modification.attribute, range: modification.range)
        }
        super.processEditing()
    }
    
    public override func setAttributes(_ attrs: [NSAttributedString.Key : Any]?, range: NSRange) {
        backingStore.setAttributes(attrs, range: range)
        self.edited(.editedAttributes, range: range, changeInLength: 0)
    }
}
