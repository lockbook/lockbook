import Foundation
#if os(iOS)
import UIKit
#elseif os(macOS)
import AppKit
#endif
import Down

public class Storage: NSTextStorage {
    
    var name: String?
    var backingStore = NSMutableAttributedString()
    var currentStyles: [AttributeRange] = []
    var myEditedRange: NSRange?
    var myChangeInLength: Int = 0
    var us: Bool = true
    var parser: Parser?
    
    /// The current incremental implementation doesn't handle emojis well. Emojis are their own font and we don't let external people
    /// modify our string attributes. We don't let them do this so users can copy paste freely without messing up formatting. Also there
    /// were many strange behaviors around the boundries of spans, # test would sometimes result in `# t` being a heading and the
    /// rest being text. Could be worth revisiting exactly how we do ranges.
    ///
    /// Could also be worth revisiting the idea of making our own parser where we have a lot of control over the ranges being generated.
    var incremental = false
    
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
        if let name = name {
            if name.hasSuffix(".txt") || name.hasSuffix(".text") {
                return
            }
        }
        
        us = true
        print()
        var startingPoint = Date()
        
        let parser = Parser(backingStore.string)
        self.parser = parser
        let newStyles = parser.processedDocument
        var dirty: Bool
        if incremental {
            print("parser perf: \(startingPoint.timeIntervalSinceNow * -1)")
            
            startingPoint = Date()
            let sameSize = currentStyles.count == newStyles.count
            dirty = !sameSize
            if sameSize {
                for (index, currentStyle) in currentStyles.enumerated() {
                    if !currentStyle.isEqual(to: newStyles[index]) {
                        dirty = true
                        break
                    }
                }
            }
        } else {
            dirty = true
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
    
    public override func setAttributes(_ attrs: [NSAttributedString.Key : Any]?, range: NSRange) {
        if us {
            backingStore.setAttributes(attrs, range: range)
        }
        edited(.editedAttributes, range: range, changeInLength: 0)
    }
}
