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
        print("range: \(range)")
        print("string: \(string)")
        print("changeInLegngth: \((string as NSString).length)")
        beginEditing()
        backingStore.replaceCharacters(in: range, with: string)
        
        let parsed = Parser(backingStore.string)
        let base = parsed.base()
        backingStore.setAttributes(base.attribute, range: base.range)
        for modification in parsed.processedDocument {
            backingStore.addAttributes(modification.attribute, range: modification.range)
        }
        
        self.edited(.editedCharacters, range: range, changeInLength: string.utf16.count - range.length)
        endEditing()
//        self.edited(.editedAttributes, range: base.range, changeInLength: 0)
    }
    
//    public override func processEditing() {
//        print("processEditing")
//        let parsed = Parser(backingStore.string)
//        let base = parsed.base()
//        backingStore.setAttributes(base.attribute, range: base.range)
//        for modification in parsed.processedDocument {
//            backingStore.addAttributes(modification.attribute, range: modification.range)
//        }
//        super.processEditing()
//    }
//    
    public override func setAttributes(_ attrs: [NSAttributedString.Key : Any]?, range: NSRange) {
        backingStore.setAttributes(attrs, range: range)
        self.edited(.editedAttributes, range: range, changeInLength: 0)
    }
}
