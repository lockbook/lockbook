//
//  Storage.swift
//  Notepad
//
//  Created by Rudd Fawcett on 10/14/16.
//  Copyright © 2016 Rudd Fawcett. All rights reserved.
//

#if os(iOS)
    import UIKit
#elseif os(macOS)
    import AppKit
#endif
//import Down

public class Storage: NSTextStorage {
    /// The Theme for the Notepad.
    public var theme: Theme? {
        didSet {
            let wholeRange = NSRange(location: 0, length: (self.string as NSString).length)

            self.beginEditing()
            self.applyStyles(wholeRange)
            self.edited(.editedAttributes, range: wholeRange, changeInLength: 0)
            self.endEditing()
        }
    }
    public var markdowner: (String) -> [MarkdownNode] = { _ in [] }
    public var applyMarkdown: (NSMutableAttributedString, MarkdownNode) -> Void = { _,_ in }
    public var applyBody: (NSMutableAttributedString, NSRange) -> Void = { _,_ in }

    /// The underlying text storage implementation.
    var backingStore = NSTextStorage()

    override public var string: String {
        get {
            return backingStore.string
        }
    }

    override public init() {
        super.init()
    }
    
    override public init(attributedString attrStr: NSAttributedString) {
        super.init(attributedString:attrStr)
        backingStore.setAttributedString(attrStr)
    }

    required public init?(coder aDecoder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }
    
    required public init(itemProviderData data: Data, typeIdentifier: String) throws {
        fatalError("init(itemProviderData:typeIdentifier:) has not been implemented")
    }
    
    #if os(macOS)
    required public init?(pasteboardPropertyList propertyList: Any, ofType type: String) {
        fatalError("init(pasteboardPropertyList:ofType:) has not been implemented")
    }
    
    required public init?(pasteboardPropertyList propertyList: Any, ofType type: NSPasteboard.PasteboardType) {
        fatalError("init(pasteboardPropertyList:ofType:) has not been implemented")
    }
    #endif

    /// Finds attributes within a given range on a String.
    ///
    /// - parameter location: How far into the String to look.
    /// - parameter range:    The range to find attributes for.
    ///
    /// - returns: The attributes on a String within a certain range.
    override public func attributes(at location: Int, longestEffectiveRange range: NSRangePointer?, in rangeLimit: NSRange) -> [NSAttributedString.Key : Any] {
        return backingStore.attributes(at: location, longestEffectiveRange: range, in: rangeLimit)
    }

    /// Replaces edited characters within a certain range with a new string.
    ///
    /// - parameter range: The range to replace.
    /// - parameter str:   The new string to replace the range with.
    override public func replaceCharacters(in range: NSRange, with str: String) {
        self.beginEditing()
        backingStore.replaceCharacters(in: range, with: str)
        let len = (str as NSString).length
        let change = len - range.length
        self.edited([.editedCharacters, .editedAttributes], range: range, changeInLength: change)
        self.endEditing()
    }

    /// Sets the attributes on a string for a particular range.
    ///
    /// - parameter attrs: The attributes to add to the string for the range.
    /// - parameter range: The range in which to add attributes.
    public override func setAttributes(_ attrs: [NSAttributedString.Key : Any]?, range: NSRange) {
        self.beginEditing()
        backingStore.setAttributes(attrs, range: range)
        self.edited(.editedAttributes, range: range, changeInLength: 0)
        self.endEditing()
    }
    
    /// Retrieves the attributes of a string for a particular range.
    ///
    /// - parameter at: The location to begin with.
    /// - parameter range: The range in which to retrieve attributes.
    public override func attributes(at location: Int, effectiveRange range: NSRangePointer?) -> [NSAttributedString.Key : Any] {
        return backingStore.attributes(at: location, effectiveRange: range)
    }

    override public func processEditing() {
        let backingString = backingStore.string
        if let nsRange = backingString.range(from: NSMakeRange(NSMaxRange(editedRange), 0)) {
            let indexRange = backingString.lineRange(for: nsRange)
            let lineRange = backingString.nsRange(from: indexRange)
            let extendedRange: NSRange = NSUnionRange(editedRange, lineRange)

            applyStyles(extendedRange)
        }
        super.processEditing()
    }

    func applyStyles(_ range: NSRange) {
        let md = markdowner(self.string)
        let attr = NSMutableAttributedString(string: self.string)
        let wholeDocument = NSRange(location: 0, length: self.string.count)
        applyBody(attr, wholeDocument)
        md.forEach {
            applyMarkdown(attr, $0)
        }
        backingStore.setAttributedString(attr)
        self.edited(.editedAttributes, range: wholeDocument, changeInLength: 0)
    }
}
