import Foundation
#if os(iOS)
import UIKit
#elseif os(macOS)
import AppKit
#endif
import Down

public class Storage: NSTextStorage {
    
    var backingStore = NSMutableAttributedString()
    var currentStyles: [AttributeRange] = []
    var myEditedRange: NSRange?
    var myChangeInLength: Int = 0
    
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
        
        myEditedRange = range
        myChangeInLength = string.utf16.count - range.length
        self.edited(.editedCharacters, range: range, changeInLength: myChangeInLength)
    }
    
    public func syntaxHighlight() {
        beginEditing()
        let startingPoint = Date()

        let parsed = Parser(backingStore.string)
        print("\(startingPoint.timeIntervalSinceNow * -1) seconds elapsed")

//        backingStore.addAttributes(base.style, range: base.range)
        for modification in parsed.processedDocument {
            backingStore.addAttributes(modification.finalizeAttributes(), range: modification.range)
            self.edited(.editedAttributes, range: modification.range, changeInLength: 0)
        }

//        DispatchQueue.main.asyncAfter(deadline: .now() + .seconds(2)) {
//            for modification in parsed.processedDocument {
//                if modification.style == .Code {
//                    let processed = modification.style.attributes()
//                    for (attribute, _) in processed {
//                        self.removeAttribute(attribute, range: modification.range)
//                    }
//                }
//            }
//        }
        
//        self.edited(.editedAttributes, range: base.range, changeInLength: 0)
        
        endEditing()
    }
    
    func adjustCurrentStyles() {
        
    }
    
    public override func setAttributes(_ attrs: [NSAttributedString.Key : Any]?, range: NSRange) {
        backingStore.setAttributes(attrs, range: range)
        self.edited(.editedAttributes, range: range, changeInLength: 0)
    }
}
