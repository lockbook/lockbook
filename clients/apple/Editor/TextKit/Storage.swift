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
        print()
        print(DebugVisitor().visit(document: (try? Down(markdownString: backingStore.string).toDocument())!))

        var startingPoint = Date()
        let newStyles = Parser(backingStore.string).processedDocument
        adjustCurrentStyles()
        print("parser perf: \(startingPoint.timeIntervalSinceNow * -1)")

        startingPoint = Date()
        let sameSize = currentStyles.count == newStyles.count
        var dirty = !sameSize
        if sameSize {
            for (index, currentStyle) in currentStyles.enumerated() {
                if !currentStyle.isEqual(to: newStyles[index]) {
                    dirty = true
                    break
                }
            }
        }
        
        if dirty {
            print("DIRT")
            currentStyles = newStyles
            beginEditing()
            
            for modification in newStyles {
                setAttributes(modification.finalizeAttributes(), range: modification.range)
            }
            endEditing()
        }
        print("doc update perf: \(startingPoint.timeIntervalSinceNow * -1)")
        print()
    }
    
    func adjustCurrentStyles() {
        
    }
    
    public override func setAttributes(_ attrs: [NSAttributedString.Key : Any]?, range: NSRange) {
        backingStore.setAttributes(attrs, range: range)
        self.edited(.editedAttributes, range: range, changeInLength: 0)
    }
}
