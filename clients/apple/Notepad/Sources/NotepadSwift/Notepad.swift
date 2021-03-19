#if os(iOS)
import UIKit

public class Notepad: UITextView, UITextViewDelegate {
    public var onTextChange: (String) -> Void = { _ in }
    public var storage: Storage = Storage()

    convenience public init(frame: CGRect, theme: Theme) {
        self.init(frame: frame, textContainer: nil)
        self.storage.theme = theme
        self.backgroundColor = theme.backgroundColor
        self.tintColor = theme.tintColor
        self.autoresizingMask = [.flexibleWidth, .flexibleHeight]
    }


    override init(frame: CGRect, textContainer: NSTextContainer?) {
        let layoutManager = NSLayoutManager()
        let containerSize = CGSize(width: frame.size.width, height: frame.size.height)
        let container = NSTextContainer(size: containerSize)
        container.widthTracksTextView = true

        layoutManager.addTextContainer(container)
        storage.addLayoutManager(layoutManager)
        super.init(frame: frame, textContainer: container)
        self.delegate = self
    }



    required public init?(coder aDecoder: NSCoder) {
        super.init(coder: aDecoder)
        let layoutManager = NSLayoutManager()
        let containerSize = CGSize(width: frame.size.width, height: CGFloat.greatestFiniteMagnitude)
        let container = NSTextContainer(size: containerSize)
        container.widthTracksTextView = true
        layoutManager.addTextContainer(container)
        storage.addLayoutManager(layoutManager)
        self.delegate = self
    }

    public func textViewDidChange(_ textView: UITextView) {
        onTextChange(textView.text)
    }

    public func styleNow() {
        self.storage.applyStyles()
    }
}
#else
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

    public func styleNow() {
        self.storage.applyStyles()
    }
}
#endif
