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
    var us: Bool = true
    var parser: Parser?

    override init() {
        super.init()
        print("INIT")
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }
    
    required init?(pasteboardPropertyList propertyList: Any, ofType type: NSPasteboard.PasteboardType) {
        fatalError("init(pasteboardPropertyList:ofType:) has not been implemented")
    }
    
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
        
        edited(.editedCharacters, range: range, changeInLength: myChangeInLength)
    }
    
    public func syntaxHighlight() {
        us = true
        print()
        var startingPoint = Date()

        let parser = Parser(backingStore.string)
        self.parser = parser
        let newStyles = parser.processedDocument
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
        us = false
    }
    
    func adjustCurrentStyles() {
        
    }
    
    public override func setAttributes(_ attrs: [NSAttributedString.Key : Any]?, range: NSRange) {
        if us {
            backingStore.setAttributes(attrs, range: range)
        }
        edited(.editedAttributes, range: range, changeInLength: 0)
    }
}
