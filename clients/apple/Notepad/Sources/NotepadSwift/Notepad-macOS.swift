//
//  Notepad-macOS.swift
//  Notepad
//
//  Created by Christian Tietze on 2017-07-21.
//  Copyright © 2017 Rudd Fawcett. All rights reserved.
//

#if os(macOS)
import AppKit

public class Notepad: NSTextView {
    public var onTextChange: (String) -> Void = { _ in }
    public var storage: Storage = Storage()

    convenience public init(frame: CGRect, theme: Theme) {
        self.init(frame: frame, textContainer: nil)
        self.storage.theme = theme
        self.backgroundColor = theme.backgroundColor
    }

    override public func didChangeText() {
        self.onTextChange(self.storage.string)
    }
}
#endif
