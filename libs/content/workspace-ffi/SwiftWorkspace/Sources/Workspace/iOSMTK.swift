#if os(iOS)
    import UIKit
    import MetalKit
    import Bridge
    import SwiftUI
    import MobileCoreServices
    import UniformTypeIdentifiers

    import GameController

    // MARK: - MdView
    public class MdView: UIView, UITextInput {
        public static let TOOL_BAR_HEIGHT: CGFloat = 42
        public static let FLOATING_CURSOR_OFFSET_HEIGHT: CGFloat = 0.6

        let mtkView: iOSMTK
        var wsHandle: UnsafeMutableRawPointer? { mtkView.wsHandle }

        // text input
        let textInteraction = UITextInteraction(for: .editable)
        public var inputDelegate: UITextInputDelegate?
        public lazy var tokenizer: UITextInputTokenizer = LBTokenizer(wsHandle: self.wsHandle)

        // pan (iPad trackpad support)
        let panRecognizer = UIPanGestureRecognizer()

        // undo/redo
        let undo = iOSUndoManager()
        public override var undoManager: UndoManager? {
            return undo
        }

        // drop
        var dropDelegate: UIDropInteractionDelegate?

        // ?
        let currentHeaderSize: Double

        // interactive refinement (floating cursor)
        var interactiveRefinementInProgress = false
        var lastFloatingCursorRect: CGRect? = nil
        var floatingCursor: UIView = UIView()
        var floatingCursorWidth = 1.0
        var floatingCursorNewStartX = 0.0
        var floatingCursorNewEndX = 0.0
        var floatingCursorNewStartY = 0.0
        var floatingCursorNewEndY = 0.0
        var autoScroll: Timer? = nil

        // range adjustment (selection handles)
        var rangeAdjustmentInProgress = false

        // gestures
        var gestureDelegate = MdGestureDelegate()

        init(mtkView: iOSMTK, headerSize: Double) {
            self.mtkView = mtkView
            self.currentHeaderSize = headerSize

            super.init(frame: .infinite)

            self.clipsToBounds = true
            self.isUserInteractionEnabled = true

            // touch
            let touch = WsTouchGestureRecognizer()
            touch.delegate = self.gestureDelegate
            touch.toCancel = []
            touch.mtkView = mtkView
            self.addGestureRecognizer(touch)
            touch.addTarget(self, action: #selector(handleWsTouch(_:)))

            // text input
            textInteraction.textInput = self
            self.addInteraction(textInteraction)

            for gestureRecognizer in self.gestureRecognizers ?? [] {
                // receive touch events immediately even if they are part of any recognized gestures
                // this supports checkboxes and other interactive markdown elements in the text area (crudely)
                gestureRecognizer.cancelsTouchesInView = false
                gestureRecognizer.delaysTouchesBegan = false
                gestureRecognizer.delaysTouchesEnded = false

                // send interactive refinements to our handler
                // this is the intended way to support a floating cursor
                if gestureRecognizer.name == "UITextInteractionNameInteractiveRefinement" {
                    gestureRecognizer.addTarget(
                        self, action: #selector(handleInteractiveRefinement(_:)))
                }

                // send range adjustments to our handler
                // this supports automatic scrolling when dragging selection handles
                if gestureRecognizer.name == "UITextInteractionNameRangeAdjustment" {
                    gestureRecognizer.addTarget(self, action: #selector(handleRangeAdjustment(_:)))
                }

                // setup cursor placement cancellation when workspace consumes a tap
                if gestureRecognizer.name == "UITextInteractionNameSingleTap" {
                    touch.toCancel?.append(gestureRecognizer)
                }
                
                // setup interactive refinement cancellation when workspace consumes a tap
                if gestureRecognizer.name == "UITextInteractionNameInteractiveRefinement" {
                    touch.toCancel?.append(gestureRecognizer)
                }
            }

            mtkView.onSelectionChanged = { [weak self] in
                self?.inputDelegate?.selectionDidChange(self)
            }
            mtkView.onTextChanged = { [weak self] in
                self?.inputDelegate?.textDidChange(self)
            }

            // pan (iPad trackpad support)
            panRecognizer.addTarget(self, action: #selector(handlePan(_:)))
            panRecognizer.allowedScrollTypesMask = .all
            panRecognizer.maximumNumberOfTouches = 0
            self.addGestureRecognizer(panRecognizer)

            // drop
            self.dropDelegate = MdDropDelegate(
                mtkView: mtkView, textDelegate: inputDelegate!, textInput: self)
            let dropInteraction = UIDropInteraction(delegate: self.dropDelegate!)
            self.addInteraction(dropInteraction)

            // undo/redo
            self.undo.textWrapper = self
            self.undo.wsHandle = self.wsHandle
            self.undo.mtkView = mtkView
            self.undo.inputDelegate = inputDelegate

            // interactive refinement (floating cursor)
            if #available(iOS 17.4, *) {
                let concreteFloatingCursor = UIStandardTextCursorView()
                concreteFloatingCursor.tintColor = .systemBlue
                floatingCursor = concreteFloatingCursor
            } else {
                floatingCursor.backgroundColor = .systemBlue
                floatingCursor.layer.cornerRadius = 1
                floatingCursorWidth = 2
            }

            floatingCursor.layer.shadowColor = UIColor.black.cgColor
            floatingCursor.layer.shadowOpacity = 0.4
            floatingCursor.layer.shadowOffset = CGSize(width: 0, height: 8)
            floatingCursor.layer.shadowRadius = 4
            floatingCursor.isHidden = true

            addSubview(floatingCursor)
        }

        @objc private func handleWsTouch(_ recognizer: WsTouchGestureRecognizer) {
            mtkView.handleWsTouch(recognizer)
        }

        @objc func handlePan(_ sender: UIPanGestureRecognizer? = nil) {
            mtkView.handleTrackpadScroll(sender)
        }

        @objc private func handleInteractiveRefinement(_ recognizer: UIGestureRecognizer) {
            switch recognizer.state {
            case .possible:
                break
            case .began:
                interactiveRefinementInProgress = true
            case .changed:
                break
            case .ended, .cancelled, .failed:
                interactiveRefinementInProgress = false

                inputDelegate?.selectionWillChange(self)
                mtkView.drawImmediately()
                inputDelegate?.selectionDidChange(self)
            default:
                break
            }
        }

        @objc private func handleRangeAdjustment(_ recognizer: UIGestureRecognizer) {
            switch recognizer.state {
            case .possible:
                return
            case .began:
                rangeAdjustmentInProgress = true
            case .changed:
                let location = recognizer.location(in: recognizer.view)
                self.scrollTo(location.y)
            case .ended, .cancelled, .failed:
                rangeAdjustmentInProgress = false
            default:
                break
            }

        }

        required init(coder: NSCoder) {
            fatalError("init(coder:) has not been implemented")
        }

        public func beginFloatingCursor(at point: CGPoint) {
            tintColor = traitCollection.userInterfaceStyle == .light ? .lightGray : .gray
            self.floatingCursorNewEndX = self.bounds.size.width
            self.floatingCursorNewEndY = self.bounds.size.height

            setFloatingCursorLoc(point: point, animate: false)
            self.bringSubviewToFront(floatingCursor)
            floatingCursor.isHidden = false
        }

        public func updateFloatingCursor(at point: CGPoint) {
            if point.x < floatingCursorNewStartX {
                floatingCursorNewEndX -= (floatingCursorNewStartX - point.x)
                floatingCursorNewStartX = point.x
            }

            if point.x > floatingCursorNewEndX {
                floatingCursorNewStartX += (point.x - floatingCursorNewEndX)
                floatingCursorNewEndX = point.x
            }

            if point.y < floatingCursorNewStartY {
                floatingCursorNewEndY -= (floatingCursorNewStartY - point.y)
                floatingCursorNewStartY = point.y
            }

            if point.y > floatingCursorNewEndY {
                floatingCursorNewStartY += (point.y - floatingCursorNewEndY)
                floatingCursorNewEndY = point.y
            }

            setFloatingCursorLoc(point: point, animate: true)
        }

        public func endFloatingCursor() {
            if let cursorRect = lastFloatingCursorRect {
                UIView.animate(
                    withDuration: 0.15,
                    animations: { [weak self] in
                        if let textInputWrapper = self {
                            textInputWrapper.floatingCursor.frame = CGRect(
                                x: cursorRect.origin.x, y: cursorRect.origin.y,
                                width: textInputWrapper.floatingCursorWidth,
                                height: cursorRect.height + Self.FLOATING_CURSOR_OFFSET_HEIGHT)
                        }
                    },
                    completion: { [weak self] finished in
                        if let textWrapper = self {
                            textWrapper.floatingCursor.isHidden = true
                            textWrapper.tintColor = .systemBlue
                            textWrapper.inputDelegate?.selectionWillChange(textWrapper)
                            textWrapper.mtkView.drawImmediately()
                            textWrapper.inputDelegate?.selectionDidChange(textWrapper)

                            textWrapper.floatingCursorNewStartX = 0
                            textWrapper.floatingCursorNewEndX = textWrapper.bounds.size.width
                            textWrapper.floatingCursorNewStartY = 0
                            textWrapper.floatingCursorNewEndY = textWrapper.bounds.size.height
                        }
                    })
            }
        }

        func setFloatingCursorLoc(point: CGPoint, animate: Bool) {
            let pos = closestPosition(to: point)
            let cursorRect = caretRect(for: pos!)

            lastFloatingCursorRect = cursorRect

            let x = point.x - self.floatingCursorNewStartX
            let y = point.y - self.floatingCursorNewStartY

            self.scrollTo(y)

            if animate {
                UIView.animate(
                    withDuration: 0.15,
                    animations: { [weak self] in
                        if let textWrapper = self {
                            textWrapper.floatingCursor.frame = CGRect(
                                x: x, y: y - (cursorRect.height / 2),
                                width: textWrapper.floatingCursorWidth,
                                height: cursorRect.height + Self.FLOATING_CURSOR_OFFSET_HEIGHT)
                        }
                    })
            } else {
                floatingCursor.frame = CGRect(
                    x: x, y: y - (cursorRect.height / 2), width: floatingCursorWidth,
                    height: cursorRect.height + Self.FLOATING_CURSOR_OFFSET_HEIGHT)
            }
        }

        func scrollTo(_ y: CGFloat) {
            let scrollUp = y >= bounds.height - 20
            let scrollDown = y <= 20

            if (scrollUp || scrollDown) && autoScroll == nil {
                autoScroll = Timer.scheduledTimer(withTimeInterval: 0.016, repeats: true) {
                    [self] timer in
                    if floatingCursor.isHidden && !rangeAdjustmentInProgress {
                        timer.invalidate()
                    }

                    mouse_moved(wsHandle, Float(bounds.width / 2), Float(bounds.height / 2))
                    scroll_wheel(wsHandle, 0, scrollUp ? -20 : 20, false, false, false, false)
                    mouse_gone(wsHandle)

                    mtkView.drawImmediately()
                }
            } else if let autoScroll,
                !scrollUp && !scrollDown
            {
                autoScroll.invalidate()
                self.autoScroll = nil
            }
        }

        public override func canPerformAction(_ action: Selector, withSender sender: Any?) -> Bool {
            if action == #selector(cut(_:)) || action == #selector(copy(_:)) {
                return selectedTextRange?.isEmpty == false
            }

            if action == #selector(paste(_:)) {
                return UIPasteboard.general.hasStrings || UIPasteboard.general.hasImages
                    || UIPasteboard.general.hasURLs
            }

            if action == #selector(replace(_:withText:)) {
                return false
            }

            if action == NSSelectorFromString("replace:") {
                return true
            }

            return super.canPerformAction(action, withSender: sender)
        }

        @objc func replace(_ sender: Any?) {
            guard let replacement = sender as? NSObject,
                let range = replacement.value(forKey: "range") as? UITextRange,
                let replacementText = replacement.value(forKey: "replacementText") as? NSString
            else {
                return
            }

            replace(range, withText: replacementText as String)
        }

        func importContent(_ importFormat: SupportedImportFormat, isPaste: Bool) {
            inputDelegate?.textWillChange(self)
            inputDelegate?.selectionWillChange(self)
            mtkView.importContent(importFormat, isPaste: isPaste)
            mtkView.drawImmediately()
            inputDelegate?.selectionDidChange(self)
            inputDelegate?.textDidChange(self)
        }

        public func insertText(_ text: String) {
            guard (markedTextRange ?? selectedTextRange) as? LBTextRange != nil,
                !text.isEmpty
            else {
                return
            }

            inputDelegate?.selectionWillChange(self)
            inputDelegate?.textWillChange(self)
            insert_text(wsHandle, text)
            mtkView.drawImmediately()
            inputDelegate?.textDidChange(self)
            inputDelegate?.selectionDidChange(self)
        }

        public func text(in range: UITextRange) -> String? {
            guard let range = (range as? LBTextRange)?.c else {
                return nil
            }
            guard let result = text_in_range(wsHandle, range) else {
                return nil
            }
            let str = String(cString: result)
            free_text(UnsafeMutablePointer(mutating: result))
            return str
        }

        public func replace(_ range: UITextRange, withText text: String) {
            guard let range = range as? LBTextRange else {
                return
            }

            inputDelegate?.textWillChange(self)
            replace_text(wsHandle, range.c, text)
            mtkView.drawImmediately()
            inputDelegate?.textDidChange(self)
        }

        public var selectedTextRange: UITextRange? {
            set {
                guard let range = (newValue as? LBTextRange)?.c else {
                    return
                }

                if !floatingCursor.isHidden || interactiveRefinementInProgress {
                    set_selected(wsHandle, range)
                    return
                }

                inputDelegate?.selectionWillChange(self)
                set_selected(wsHandle, range)
                mtkView.drawImmediately()
                inputDelegate?.selectionDidChange(self)
            }

            get {
                let range = get_selected(wsHandle)
                if range.none {
                    return nil
                }
                return LBTextRange(c: range)
            }
        }

        public var markedTextRange: UITextRange? {
            let range = get_marked(wsHandle)
            if range.none {
                return nil
            }
            return LBTextRange(c: range)
        }

        public var markedTextStyle: [NSAttributedString.Key: Any]? {
            set {
                unimplemented()
            }

            get {
                unimplemented()
                return nil
            }
        }

        public func setMarkedText(_ markedText: String?, selectedRange: NSRange) {
            guard (markedTextRange ?? selectedTextRange) as? LBTextRange != nil else {
                return
            }

            inputDelegate?.textWillChange(self)
            set_marked(
                wsHandle,
                CTextRange(
                    none: false,
                    start: CTextPosition(none: false, pos: UInt(selectedRange.lowerBound)),
                    end: CTextPosition(none: false, pos: UInt(selectedRange.upperBound))),
                markedText)
            mtkView.drawImmediately()
            inputDelegate?.textDidChange(self)
        }

        public func unmarkText() {
            guard markedTextRange as? LBTextRange != nil else {
                return
            }

            inputDelegate?.textWillChange(self)
            unmark_text(wsHandle)
            mtkView.drawImmediately()
            inputDelegate?.textDidChange(self)
        }

        public var beginningOfDocument: UITextPosition {
            let res = beginning_of_document(wsHandle)
            return LBTextPos(c: res)
        }

        public var endOfDocument: UITextPosition {
            let res = end_of_document(wsHandle)
            return LBTextPos(c: res)
        }

        public func textRange(from fromPosition: UITextPosition, to toPosition: UITextPosition)
            -> UITextRange?
        {
            guard let start = (fromPosition as? LBTextPos)?.c else {
                return nil
            }
            let end = (toPosition as! LBTextPos).c
            let range = text_range(start, end)

            if range.none {
                return nil
            } else {
                return LBTextRange(c: range)
            }
        }

        public func position(from position: UITextPosition, offset: Int) -> UITextPosition? {
            guard let start = (position as? LBTextPos)?.c else {
                return nil
            }
            let new = position_offset(wsHandle, start, Int32(offset))
            if new.none {
                return nil
            }
            return LBTextPos(c: new)
        }

        public func position(
            from position: UITextPosition, in direction: UITextLayoutDirection, offset: Int
        ) -> UITextPosition? {
            let start = (position as! LBTextPos).c
            let direction = CTextLayoutDirection(rawValue: UInt32(direction.rawValue))
            let new = position_offset_in_direction(wsHandle, start, direction, Int32(offset))
            if new.none {
                return nil
            }
            return LBTextPos(c: new)
        }

        public func compare(_ position: UITextPosition, to other: UITextPosition)
            -> ComparisonResult
        {
            guard let left = (position as? LBTextPos)?.c.pos,
                let right = (other as? LBTextPos)?.c.pos
            else {
                return ComparisonResult.orderedAscending
            }

            let res: ComparisonResult
            if left < right {
                res = ComparisonResult.orderedAscending
            } else if left == right {
                res = ComparisonResult.orderedSame
            } else {
                res = ComparisonResult.orderedDescending
            }
            return res
        }

        public func offset(from: UITextPosition, to toPosition: UITextPosition) -> Int {

            guard let left = (from as? LBTextPos)?.c.pos,
                let right = (toPosition as? LBTextPos)?.c.pos
            else {
                return 0
            }

            return Int(right) - Int(left)
        }

        public func position(within range: UITextRange, farthestIn direction: UITextLayoutDirection)
            -> UITextPosition?
        {
            unimplemented()
            return nil
        }

        public func characterRange(
            byExtending position: UITextPosition, in direction: UITextLayoutDirection
        ) -> UITextRange? {
            unimplemented()
            return nil
        }

        public func baseWritingDirection(
            for position: UITextPosition, in direction: UITextStorageDirection
        ) -> NSWritingDirection {
            return NSWritingDirection.leftToRight
        }

        public func setBaseWritingDirection(
            _ writingDirection: NSWritingDirection, for range: UITextRange
        ) {
            if writingDirection != .leftToRight {
                unimplemented()
            }
        }

        public func firstRect(for range: UITextRange) -> CGRect {
            let range = (range as! LBTextRange).c
            let result = first_rect(wsHandle, range)
            return CGRect(
                x: result.min_x, y: result.min_y - mtkView.docHeaderSize,
                width: result.max_x - result.min_x, height: result.max_y - result.min_y)
        }

        public func caretRect(for position: UITextPosition) -> CGRect {
            let position = (position as! LBTextPos).c
            let result = cursor_rect_at_position(wsHandle, position)
            return CGRect(
                x: result.min_x, y: result.min_y - mtkView.docHeaderSize, width: 1,
                height: result.max_y - result.min_y)
        }

        public func selectionRects(for range: UITextRange) -> [UITextSelectionRect] {
            let range = (range as! LBTextRange).c
            let result = selection_rects(wsHandle, range)

            let buffer = Array(UnsafeBufferPointer(start: result.rects, count: Int(result.size)))

            free_selection_rects(result)

            let selectionRects: [UITextSelectionRect] = buffer.enumerated().map { (index, rect) in
                let new_rect = CRect(
                    min_x: rect.min_x, min_y: rect.min_y - mtkView.docHeaderSize, max_x: rect.max_x,
                    max_y: rect.max_y - mtkView.docHeaderSize)

                return LBTextSelectionRect(cRect: new_rect, loc: index, size: buffer.count)
            }

            return selectionRects
        }

        public func closestPosition(to point: CGPoint) -> UITextPosition? {
            let (x, y) =
                floatingCursor.isHidden
                ? (point.x, point.y)
                : (point.x - floatingCursorNewStartX, point.y - floatingCursorNewStartY)

            let point = CPoint(x: x, y: y + mtkView.docHeaderSize)
            let result = position_at_point(wsHandle, point)

            return LBTextPos(c: result)
        }

        public func closestPosition(to point: CGPoint, within range: UITextRange) -> UITextPosition?
        {
            unimplemented()
            return nil
        }

        public func characterRange(at point: CGPoint) -> UITextRange? {
            unimplemented()
            return nil
        }

        public var hasText: Bool {
            let res = has_text(wsHandle)
            return res
        }
        
        public var isEditable: Bool {
            get {
                is_current_tab_editable(wsHandle)
            }
        }
        

        public func deleteBackward() {
            if !hasText {
                return
            }

            guard (markedTextRange ?? selectedTextRange) as? LBTextRange != nil else {
                return
            }

            inputDelegate?.selectionWillChange(self)
            inputDelegate?.textWillChange(self)
            backspace(wsHandle)
            mtkView.drawImmediately()
            inputDelegate?.textDidChange(self)
            inputDelegate?.selectionDidChange(self)
        }

        public override func cut(_ sender: Any?) {
            guard let range = (markedTextRange ?? selectedTextRange) as? LBTextRange,
                !range.isEmpty
            else {
                return
            }

            inputDelegate?.textWillChange(self)
            inputDelegate?.selectionWillChange(self)
            clipboard_cut(self.wsHandle)
            mtkView.drawImmediately()
            inputDelegate?.selectionDidChange(self)
            inputDelegate?.textDidChange(self)
        }

        public override func copy(_ sender: Any?) {
            clipboard_copy(self.wsHandle)
            setNeedsDisplay(self.frame)
        }

        public override func paste(_ sender: Any?) {
            if let image = UIPasteboard.general.image {
                importContent(.image(image), isPaste: true)
            } else if let string = UIPasteboard.general.string {
                importContent(.text(string), isPaste: true)
            }
        }

        public override func selectAll(_ sender: Any?) {
            if !hasText {
                return
            }

            inputDelegate?.selectionWillChange(self)
            select_all(self.wsHandle)
            mtkView.drawImmediately()
            inputDelegate?.selectionDidChange(self)
        }

        func getText() -> String {
            let result = get_text(wsHandle)
            let str = String(cString: result!)
            free_text(UnsafeMutablePointer(mutating: result))
            return str
        }

        public override var canBecomeFirstResponder: Bool {
            return true
        }

        override public var keyCommands: [UIKeyCommand]? {
            let deleteWord = UIKeyCommand(
                input: UIKeyCommand.inputDelete, modifierFlags: [.alternate],
                action: #selector(deleteWord))

            deleteWord.wantsPriorityOverSystemBehavior = true

            return [
                deleteWord
            ]
        }

        @objc func deleteWord() {
            inputDelegate?.selectionWillChange(self)
            inputDelegate?.textWillChange(self)
            delete_word(wsHandle)
            mtkView.drawImmediately()
            inputDelegate?.textDidChange(self)
            inputDelegate?.selectionDidChange(self)
        }

        public override func pressesBegan(_ presses: Set<UIPress>, with event: UIPressesEvent?) {
            let forward = mtkView.handleKeyEvent(presses, with: event, pressBegan: true)
            if forward {
                super.pressesBegan(presses, with: event)
            }
        }

        public override func pressesEnded(_ presses: Set<UIPress>, with event: UIPressesEvent?) {
            let forward = mtkView.handleKeyEvent(presses, with: event, pressBegan: false)
            if forward {
                super.pressesEnded(presses, with: event)
            }
        }

        func unimplemented() {
            print("unimplemented!")
            Thread.callStackSymbols.forEach { print($0) }
        }
    }

    // MARK: - MdGestureDelegate
    public class MdGestureDelegate: NSObject, UIGestureRecognizerDelegate {
        public func gestureRecognizer(
            _ gestureRecognizer: UIGestureRecognizer,
            shouldRecognizeSimultaneouslyWith otherGestureRecognizer: UIGestureRecognizer
        ) -> Bool {
            // allow text interaction taps to also pass through to workspace
            // todo: this is what allows tapping a checkbox to also place the cursor
            return true
        }

        public func gestureRecognizer(
            _ gestureRecognizer: UIGestureRecognizer,
            shouldRequireFailureOf otherGestureRecognizer: UIGestureRecognizer
        ) -> Bool {
            if otherGestureRecognizer.name == "UITextInteractionNameInteractiveRefinement" {
                return true
            }
            if otherGestureRecognizer.name == "UITextInteractionNameRangeAdjustment" {
                return true
            }

            return false
        }
    }

    // MARK: - MdDropDelegate
    public class MdDropDelegate: NSObject, UIDropInteractionDelegate {
        weak var mtkView: iOSMTK?
        weak var textDelegate: UITextInputDelegate?
        weak var textInput: UITextInput?

        public init(mtkView: iOSMTK, textDelegate: UITextInputDelegate, textInput: UITextInput) {
            self.mtkView = mtkView
            self.textDelegate = textDelegate
            self.textInput = textInput
        }

        public func dropInteraction(
            _ interaction: UIDropInteraction, canHandle session: UIDropSession
        ) -> Bool {
            guard session.items.count == 1 else { return false }

            return session.hasItemsConforming(toTypeIdentifiers: [
                UTType.image.identifier, UTType.fileURL.identifier, UTType.text.identifier,
            ])
        }

        public func dropInteraction(
            _ interaction: UIDropInteraction, sessionDidUpdate session: UIDropSession
        ) -> UIDropProposal {
            return UIDropProposal(operation: .copy)
        }

        public func dropInteraction(
            _ interaction: UIDropInteraction, performDrop session: UIDropSession
        ) {
            guard let mtkView = mtkView else { return }
            guard let textDelegate = textDelegate else { return }

            textDelegate.textWillChange(textInput)
            textDelegate.selectionWillChange(textInput)

            if session.hasItemsConforming(toTypeIdentifiers: [UTType.image.identifier as String]) {
                session.loadObjects(ofClass: UIImage.self) { imageItems in
                    let images = imageItems as? [UIImage] ?? []
                    for image in images {
                        mtkView.importContent(.image(image), isPaste: true)
                    }
                }
            }

            if session.hasItemsConforming(toTypeIdentifiers: [UTType.text.identifier as String]) {
                session.loadObjects(ofClass: NSAttributedString.self) { textItems in
                    let attributedStrings = textItems as? [NSAttributedString] ?? []
                    for attributedString in attributedStrings {
                        mtkView.importContent(.text(attributedString.string), isPaste: true)
                    }
                }
            }

            if session.hasItemsConforming(toTypeIdentifiers: [UTType.fileURL.identifier as String])
            {
                _ = session.loadObjects(ofClass: URL.self) { urlItems in
                    for url in urlItems {
                        mtkView.importContent(.url(url), isPaste: true)
                    }
                }
            }

            mtkView.drawImmediately()
            textDelegate.selectionDidChange(textInput)
            textDelegate.textDidChange(textInput)
        }
    }

    // MARK: - SvgView
    public class SvgView: UIView {
        public static let TOOL_BAR_HEIGHT: CGFloat = 50

        let mtkView: iOSMTK
        var wsHandle: UnsafeMutableRawPointer? { mtkView.wsHandle }

        // pointer
        var pointerInteraction: UIPointerInteraction?

        // pencil
        var pencilDelegate: SvgPencilDelegate?
        var pencilInteraction: UIPencilInteraction?

        // gestures
        var gestureDelegate: SvgGestureDelegate?
        var tapRecognizer: UITapGestureRecognizer?
        var panRecognizer: UIPanGestureRecognizer?
        var pinchRecognizer: UIPinchGestureRecognizer?

        // menu
        var menuDelegate: SvgMenuDelegate?
        var menuInteraction: UIEditMenuInteraction?

        // ?
        let currentHeaderSize: Double

        init(mtkView: iOSMTK, headerSize: Double) {
            self.mtkView = mtkView
            self.currentHeaderSize = headerSize

            super.init(frame: .infinite)

            isMultipleTouchEnabled = true

            // pointer
            let pointerInteraction = UIPointerInteraction(delegate: mtkView.pointerDelegate)

            self.addInteraction(pointerInteraction)

            self.pointerInteraction = pointerInteraction

            // pencil
            let pencilDelegate = SvgPencilDelegate(mtkView: mtkView)
            let pencilInteraction = UIPencilInteraction()

            pencilInteraction.delegate = pencilDelegate
            addInteraction(pencilInteraction)

            self.pencilDelegate = pencilDelegate
            self.pencilInteraction = pencilInteraction

            // gestures
            self.gestureDelegate = SvgGestureDelegate()

            // gestures: touch
            let touch = WsTouchGestureRecognizer()
            touch.addTarget(self, action: #selector(handleWsTouch(_:)))
            touch.delegate = self.gestureDelegate
            touch.cancelsTouchesInView = false

            self.addGestureRecognizer(touch)

            // gestures: tap
            let tap = UITapGestureRecognizer(target: self, action: #selector(self.handleTap(_:)))
            tap.name = "SvgTap"
            tap.delegate = self.gestureDelegate
            tap.allowedTouchTypes = [
                NSNumber(value: UITouch.TouchType.direct.rawValue),
                NSNumber(value: UITouch.TouchType.indirect.rawValue),
                // NSNumber(value: UITouch.TouchType.pencil.rawValue),
                NSNumber(value: UITouch.TouchType.indirectPointer.rawValue),
            ]
            tap.numberOfTouchesRequired = 1
            tap.cancelsTouchesInView = false

            self.addGestureRecognizer(tap)

            self.tapRecognizer = tap

            // gestures: pan
            let pan = SvgPanGestureRecognizer(
                target: self, action: #selector(self.handlePan(_:)), wsTouch: touch)
            pan.allowedTouchTypes = [
                NSNumber(value: UITouch.TouchType.direct.rawValue),
                NSNumber(value: UITouch.TouchType.indirect.rawValue),
                // NSNumber(value: UITouch.TouchType.pencil.rawValue),
                NSNumber(value: UITouch.TouchType.indirectPointer.rawValue),
            ]
            pan.allowedScrollTypesMask = .all
            if UIPencilInteraction.prefersPencilOnlyDrawing {  // todo: update when prefersPencilOnlyDrawing changes
                pan.minimumNumberOfTouches = 1
            } else {
                pan.minimumNumberOfTouches = 2
            }
            pan.cancelsTouchesInView = false

            pan.delegate = gestureDelegate
            self.addGestureRecognizer(pan)

            self.panRecognizer = pan

            // gestures: pinch
            let pinch = SvgPinchGestureRecognizer(
                target: self, action: #selector(self.handlePinch(_:)), wsTouch: touch)
            pinch.cancelsTouchesInView = false

            pinch.delegate = gestureDelegate
            self.addGestureRecognizer(pinch)

            self.pinchRecognizer = pinch

            // menu
            let menuDelegate = SvgMenuDelegate()
            let menuInteraction = UIEditMenuInteraction(delegate: menuDelegate)

            self.addInteraction(menuInteraction)

            self.menuDelegate = menuDelegate
            self.menuInteraction = menuInteraction
        }

        @objc private func handleWsTouch(_ recognizer: WsTouchGestureRecognizer) {
            mtkView.handleWsTouch(recognizer)
        }

        @objc private func handleTap(_ gesture: UITapGestureRecognizer) {
            guard let menuInteraction = menuInteraction else { return }

            if gesture.state != .ended { return }

            if self.mtkView.kineticTimer != nil {
                self.mtkView.kineticTimer?.invalidate()
                self.mtkView.kineticTimer = nil
                return
            }

            // Check if we have any valid actions before presenting
            let location = gesture.location(in: self.mtkView)
            if will_consume_touch(
                self.wsHandle, Float(location.x),
                Float(location.y + SvgView.TOOL_BAR_HEIGHT))
                || (!UIPasteboard.general.hasStrings && !UIPasteboard.general.hasImages)
            {
                return
            }

            let config = UIEditMenuConfiguration(
                identifier: nil, sourcePoint: gesture.location(in: self))
            menuInteraction.presentEditMenu(with: config)
        }

        public override func canPerformAction(_ action: Selector, withSender sender: Any?) -> Bool {
            if action == #selector(paste(_:)) {
                return UIPasteboard.general.hasStrings || UIPasteboard.general.hasImages
            }

            return false
        }

        @objc func handlePan(_ sender: UIPanGestureRecognizer? = nil) {
            mtkView.handlePan(sender)
        }

        @objc func handlePinch(_ sender: UIPinchGestureRecognizer? = nil) {
            guard let event = sender, event.state != .cancelled, event.state != .failed else {
                return
            }

            if self.mtkView.kineticTimer != nil {
                self.mtkView.kineticTimer?.invalidate()
                self.mtkView.kineticTimer = nil
            }

            let scale = event.scale
            let pinchCenter = event.location(in: self.mtkView)

            if event.state == .changed {
                let zoomDelta = Float(scale)

                zoom(self.wsHandle, zoomDelta)

                let viewCenter = CGPoint(x: self.mtkView.bounds.midX, y: self.mtkView.bounds.midY)
                let offsetX = pinchCenter.x - viewCenter.x
                let offsetY = pinchCenter.y - viewCenter.y

                let panX = offsetX * (scale - 1.0)
                let panY = offsetY * (scale - 1.0)

                pan(self.wsHandle, Float(panX), Float(panY))

                event.scale = 1.0

            }
        }

        public override func paste(_ sender: Any?) {
            if let image = UIPasteboard.general.image {
                mtkView.importContent(.image(image), isPaste: true)
            } else if let string = UIPasteboard.general.string {
                mtkView.importContent(.text(string), isPaste: true)
            }
        }

        required init(coder: NSCoder) {
            fatalError("init(coder:) has not been implemented")
        }
    }

    // MARK: - SvgGestureDelegate
    public class SvgGestureDelegate: NSObject, UIGestureRecognizerDelegate {
        public func gestureRecognizer(
            _ gestureRecognizer: UIGestureRecognizer,
            shouldRecognizeSimultaneouslyWith otherGestureRecognizer: UIGestureRecognizer
        ) -> Bool {
            // Allow pan, pinch, and ws pan (touch) to work together
            // pan and pinch are configured to cancel touch when they begin
            var result = false
            switch gestureRecognizer.name {
            case "SvgPan", "SvgPinch", "SvgTap", "WsTouch":
                switch otherGestureRecognizer.name {
                case "SvgPan", "SvgPinch", "SvgTap", "WsTouch":
                    result = true
                default:
                    result = false
                }
            default:
                result = false
            }

            return result
        }
    }

    // MARK: - SvgPencilDelegate
    public class SvgPencilDelegate: NSObject, UIPencilInteractionDelegate {
        weak var mtkView: iOSMTK?

        init(mtkView: iOSMTK) {
            self.mtkView = mtkView
        }

        @available(iOS 17.5, *)
        public func pencilInteraction(
            _ interaction: UIPencilInteraction,
            didReceiveSqueeze squeeze: UIPencilInteraction.Squeeze
        ) {
            guard let mtkView = mtkView else { return }

            if squeeze.phase == .ended {
                show_tool_popover_at_cursor(mtkView.wsHandle)
            }
        }

        public func pencilInteractionDidTap(_ interaction: UIPencilInteraction) {
            guard let mtkView = mtkView else { return }

            switch UIPencilInteraction.preferredTapAction {
            case .ignore, .showColorPalette, .showInkAttributes:
                print("do nothing")
            case .switchEraser:
                toggle_drawing_tool_between_eraser(mtkView.wsHandle)
            case .switchPrevious:
                toggle_drawing_tool(mtkView.wsHandle)
            default:
                print("don't know, do nothing")
            }

            mtkView.setNeedsDisplay(mtkView.frame)
        }
    }

    // MARK: - SvgEditMenuDelegate
    public class SvgMenuDelegate: NSObject, UIEditMenuInteractionDelegate {}

    // MARK: - iOSMTKViewDelegate
    public class iOSMTKViewDelegate: NSObject, MTKViewDelegate {
        weak var mtkView: iOSMTK?

        init(mtkView: iOSMTK) {
            self.mtkView = mtkView
        }

        public func mtkView(_ view: MTKView, drawableSizeWillChange size: CGSize) {
            guard let mtkView = self.mtkView else { return }
            let wsHandle = mtkView.wsHandle

            resize_editor(
                wsHandle, Float(size.width), Float(size.height), Float(mtkView.contentScaleFactor))
            mtkView.setNeedsDisplay()
        }

        public func draw(in view: MTKView) {
            guard let mtkView = self.mtkView else { return }
            let wsHandle = mtkView.wsHandle

            if mtkView.tabSwitchTask != nil {
                mtkView.tabSwitchTask!()
                mtkView.tabSwitchTask = nil
            }

            dark_mode(wsHandle, mtkView.isDarkMode())
            show_hide_tabs(wsHandle, !mtkView.isCompact())
            set_scale(wsHandle, Float(mtkView.contentScaleFactor))

            let output = ios_frame(wsHandle)

            if output.tabs_changed {
                mtkView.workspaceOutput?.tabCount = Int(tab_count(wsHandle))
            }

            if output.selected_folder_changed {
                let selectedFolder = UUID(uuid: get_selected_folder(wsHandle)._0)
                if selectedFolder.isNil() {
                    mtkView.workspaceOutput?.selectedFolder = nil
                } else {
                    mtkView.workspaceOutput?.selectedFolder = selectedFolder
                }
            }

            let selectedFile = UUID(uuid: output.selected_file._0)
            if !selectedFile.isNil() {
                if mtkView.currentOpenDoc != selectedFile {
                    mtkView.onSelectionChanged?()
                    mtkView.onTextChanged?()
                }

                mtkView.currentOpenDoc = selectedFile

                if selectedFile != mtkView.workspaceOutput?.openDoc {
                    mtkView.workspaceOutput?.openDoc = selectedFile
                }
            }

            let currentTab = WorkspaceTab(rawValue: Int(current_tab(wsHandle)))!

            if currentTab != mtkView.workspaceOutput!.currentTab {
                DispatchQueue.main.async {
                    mtkView.workspaceOutput!.currentTab = currentTab
                }
            }

            if currentTab == .Welcome && mtkView.currentOpenDoc != nil {
                mtkView.currentOpenDoc = nil
                mtkView.workspaceOutput?.openDoc = nil
            }

            if let currentWrapper = mtkView.currentWrapper as? MdView,
                currentTab == .Markdown
            {
                if output.has_virtual_keyboard_shown && !output.virtual_keyboard_shown
                    && currentWrapper.floatingCursor.isHidden
                {
                    UIApplication.shared.sendAction(
                        #selector(UIResponder.resignFirstResponder), to: nil, from: nil, for: nil)
                }

                if output.scroll_updated {
                    mtkView.onSelectionChanged?()
                }

                if output.text_updated && !mtkView.ignoreTextUpdate {
                    mtkView.onTextChanged?()
                }

                if output.selection_updated && !mtkView.ignoreSelectionUpdate {
                    mtkView.onSelectionChanged?()
                }

                let keyboard_shown = currentWrapper.isFirstResponder && GCKeyboard.coalesced == nil
                update_virtual_keyboard(wsHandle, keyboard_shown)
            }

            if output.tab_title_clicked {
                mtkView.workspaceOutput?.renameOpenDoc = ()

                if !mtkView.isCompact() {
                    unfocus_title(wsHandle)
                }
            }

            //      FIXME:  Can we just do this in rust?
            let newFile = UUID(uuid: output.doc_created._0)
            if !newFile.isNil() {
                mtkView.workspaceInput?.openFile(id: newFile)
            }

            if output.new_folder_btn_pressed {
                mtkView.workspaceOutput?.newFolderButtonPressed = ()
            }

            if let openedUrl = output.url_opened {
                let url = textFromPtr(s: openedUrl)

                if let url = URL(string: url),
                    UIApplication.shared.canOpenURL(url)
                {
                    mtkView.workspaceOutput?.urlOpened = url
                }
            }

            if let text = output.copied_text {
                let text = textFromPtr(s: text)
                if !text.isEmpty {
                    UIPasteboard.general.string = text
                }
            }

            mtkView.redrawTask?.cancel()
            mtkView.isPaused = output.redraw_in > 50
            if mtkView.isPaused {
                let redrawIn = UInt64(truncatingIfNeeded: output.redraw_in)
                let redrawInInterval = DispatchTimeInterval.milliseconds(
                    Int(truncatingIfNeeded: min(500, redrawIn)))

                let newRedrawTask = DispatchWorkItem {
                    mtkView.drawImmediately()
                }
                DispatchQueue.main.asyncAfter(
                    deadline: .now() + redrawInInterval, execute: newRedrawTask)
                mtkView.redrawTask = newRedrawTask
            }

            mtkView.enableSetNeedsDisplay = mtkView.isPaused
        }
    }

    // MARK: - iOSPointerDelegate
    public class iOSPointerDelegate: NSObject, UIPointerInteractionDelegate {
        weak var mtkView: iOSMTK?

        init(mtkView: iOSMTK) {
            self.mtkView = mtkView
        }

        public func pointerInteraction(
            _ interaction: UIPointerInteraction, regionFor request: UIPointerRegionRequest,
            defaultRegion: UIPointerRegion
        ) -> UIPointerRegion? {
            guard let mtkView = self.mtkView else { return defaultRegion }
            let wsHandle = mtkView.wsHandle

            let offsetY: CGFloat =
                if interaction.view is MdView
                    || interaction.view is SvgView
                {
                    mtkView.docHeaderSize
                } else {
                    0
                }

            mouse_moved(wsHandle, Float(request.location.x), Float(request.location.y + offsetY))
            return defaultRegion
        }

        public func pointerInteraction(
            _ interaction: UIPointerInteraction, willEnter region: UIPointerRegion,
            animator: any UIPointerInteractionAnimating
        ) {
            guard let mtkView = self.mtkView else { return }

            mtkView.cursorTracked = true
        }

        public func pointerInteraction(
            _ interaction: UIPointerInteraction, willExit region: UIPointerRegion,
            animator: any UIPointerInteractionAnimating
        ) {
            guard let mtkView = self.mtkView else { return }

            mtkView.cursorTracked = false
            mouse_gone(mtkView.wsHandle)
        }
    }

    // MARK: - iOSMTK
    public class iOSMTK: MTKView {
        public static let TAB_BAR_HEIGHT: CGFloat = 40
        public static let TITLE_BAR_HEIGHT: CGFloat = 33
        public static let POINTER_DECELERATION_RATE: CGFloat = 0.95

        public var wsHandle: UnsafeMutableRawPointer?
        weak var currentWrapper: UIView? = nil

        // touch
        var touchRecognizer: WsTouchGestureRecognizer?
        var touchDelegate: SvgGestureDelegate?

        // pointer
        var pointerInteraction: UIPointerInteraction?
        var pointerDelegate: UIPointerInteractionDelegate?

        // mtk
        var mtkDelegate: iOSMTKViewDelegate?
        var redrawTask: DispatchWorkItem? = nil

        // workspace
        var workspaceOutput: WorkspaceOutputState?
        var workspaceInput: WorkspaceInputState?
        var currentOpenDoc: UUID? = nil  // todo: duplicated in ws output
        var currentSelectedFolder: UUID? = nil  // duplicated in ws output

        // view hierarchy management
        var tabSwitchTask: (() -> Void)? = nil  // facilitates switching wrapper views in response to tab change
        var onSelectionChanged: (() -> Void)? = nil  // only populated when wrapper is markdown
        var onTextChanged: (() -> Void)? = nil  // also only populated when wrapper is markdown
        var ignoreSelectionUpdate = false  // don't invoke corresponding handler when drawing immediately
        var ignoreTextUpdate = false  // also don't invoke corresponding handler when drawing immediately
        var docHeaderSize: Double {
            return !isCompact() ? iOSMTK.TAB_BAR_HEIGHT : 0
        }

        // kinetic scroll
        var cursorTracked = false
        var scrollSensitivity = 50.0
        var scrollId = 0
        var kineticTimer: Timer?

        override init(frame frameRect: CGRect, device: MTLDevice?) {
            super.init(frame: frameRect, device: device)

            // touch
            // this one sits on a view behind md and svg views
            // its only exposed over the md toolbar, which is not covered by md view to prevent text interactions there
            self.touchDelegate = SvgGestureDelegate()
            let touch = WsTouchGestureRecognizer()

            self.addGestureRecognizer(touch)
            touch.delegate = self.touchDelegate
            touch.addTarget(self, action: #selector(handleWsTouch(_:)))
            touch.cancelsTouchesInView = false

            self.touchRecognizer = touch

            // pointer
            let pointerDelegate = iOSPointerDelegate(mtkView: self)
            let pointer = UIPointerInteraction(delegate: pointerDelegate)

            self.addInteraction(pointer)

            self.pointerDelegate = pointerDelegate
            self.pointerInteraction = pointer

            // mtk
            self.mtkDelegate = iOSMTKViewDelegate(mtkView: self)

            self.isPaused = false
            self.enableSetNeedsDisplay = false
            self.delegate = mtkDelegate
            self.preferredFramesPerSecond = 120
            self.isUserInteractionEnabled = true
        }

        required init(coder: NSCoder) {
            fatalError("init(coder:) has not been implemented")
        }

        @objc func handleWsTouch(_ recognizer: WsTouchGestureRecognizer) {
            guard let touch = recognizer.touch else { return }

            switch recognizer.state {
            case .possible:
                break
            case .began:
                set_pencil_only_drawing(wsHandle, UIPencilInteraction.prefersPencilOnlyDrawing)

                if kineticTimer != nil {
                    kineticTimer?.invalidate()
                }

                let id = self.touchId(for: touch)

                let location = touch.preciseLocation(in: self)
                let force = touch.force != 0 ? touch.force / touch.maximumPossibleForce : 0

                touches_began(wsHandle, id, Float(location.x), Float(location.y), Float(force))

                drawImmediately()
            case .changed:
                guard let event = recognizer.event else { return }

                let id = self.touchId(for: touch)

                for touch in event.coalescedTouches(for: touch)! {
                    let location = touch.preciseLocation(in: self)
                    let force = self.force(for: touch)

                    touches_moved(
                        wsHandle, id, Float(location.x), Float(location.y), Float(force))
                }

                for touch in event.predictedTouches(for: touch)! {
                    let location = touch.preciseLocation(in: self)
                    let force = touch.force != 0 ? touch.force / touch.maximumPossibleForce : 0

                    touches_predicted(
                        wsHandle, id, Float(location.x), Float(location.y), Float(force))
                }

                drawImmediately()
            case .ended:
                let id = self.touchId(for: touch)
                let location = touch.preciseLocation(in: self)
                let force = self.force(for: touch)

                touches_ended(wsHandle, id, Float(location.x), Float(location.y), Float(force))

                drawImmediately()
            case .cancelled:
                let id = self.touchId(for: touch)
                let location = touch.preciseLocation(in: self)
                let force = self.force(for: touch)

                touches_cancelled(
                    wsHandle, id, Float(location.x), Float(location.y), Float(force))

                drawImmediately()
            case .failed:
                break
            default:
                break
            }
        }

        private func touchId(for touch: UITouch) -> UInt64 {
            let pointer = Unmanaged.passUnretained(touch).toOpaque()
            return UInt64(UInt(bitPattern: pointer))
        }

        private func force(for touch: UITouch) -> CGFloat {
            touch.force != 0 ? touch.force / touch.maximumPossibleForce : 0
        }

        @objc func handleTrackpadScroll(_ sender: UIPanGestureRecognizer? = nil) {
            guard let event = sender, event.state != .cancelled, event.state != .failed else {
                return
            }

            var velocity = event.velocity(in: self)

            velocity.x /= 50
            velocity.y /= 50

            if event.state == .ended {
                let currentScrollId = Int(Date().timeIntervalSince1970)
                scrollId = currentScrollId

                Timer.scheduledTimer(withTimeInterval: 0.016, repeats: true) { [self] timer in
                    if currentScrollId != scrollId {
                        timer.invalidate()
                        return
                    }

                    velocity.x *= Self.POINTER_DECELERATION_RATE
                    velocity.y *= Self.POINTER_DECELERATION_RATE

                    if abs(velocity.x) < 0.1 && abs(velocity.y) < 0.1 {
                        timer.invalidate()
                        return
                    }

                    if !cursorTracked {
                        mouse_moved(wsHandle, Float(bounds.width / 2), Float(bounds.height / 2))
                    }
                    scroll_wheel(
                        self.wsHandle, Float(velocity.x), Float(velocity.y), false, false, false,
                        false)
                    if !cursorTracked {
                        mouse_gone(wsHandle)
                    }

                    self.setNeedsDisplay()
                }
            } else {
                if !cursorTracked {
                    mouse_moved(wsHandle, Float(bounds.width / 2), Float(bounds.height / 2))
                }
                scroll_wheel(
                    wsHandle, Float(velocity.x), Float(velocity.y), false, false, false, false)
                if !cursorTracked {
                    mouse_gone(wsHandle)
                }
            }

            self.setNeedsDisplay()
        }

        // used in canvas
        @objc func handlePan(_ sender: UIPanGestureRecognizer? = nil) {
            guard let event = sender, event.state != .cancelled, event.state != .failed else {
                return
            }

            if event.state == .began {
                kineticTimer?.invalidate()
                kineticTimer = nil
            }

            var velocity = event.velocity(in: self)

            velocity.x /= scrollSensitivity
            velocity.y /= scrollSensitivity

            if event.state == .ended {
                let currentScrollId = Int(Date().timeIntervalSince1970)
                scrollId = currentScrollId
                touches_ended(wsHandle, UInt64.random(in: UInt64.min...UInt64.max), 0.0, 0.0, 0.0)

                kineticTimer = Timer.scheduledTimer(withTimeInterval: 0.016, repeats: true) {
                    [weak self] timer in
                    guard let self = self else {
                        timer.invalidate()
                        self?.kineticTimer = nil
                        return
                    }

                    velocity.x *= Self.POINTER_DECELERATION_RATE
                    velocity.y *= Self.POINTER_DECELERATION_RATE

                    if abs(velocity.x) < 0.1 && abs(velocity.y) < 0.1 {
                        timer.invalidate()
                        self.kineticTimer = nil
                        return
                    }

                    pan(self.wsHandle, Float(velocity.x), Float(velocity.y))
                    mouse_gone(self.wsHandle)

                    self.setNeedsDisplay()
                }
            } else {
                let translation = event.translation(in: self)

                pan(self.wsHandle, Float(translation.x), Float(translation.y))
                mouse_moved(
                    wsHandle, Float(event.location(in: self).x), Float(event.location(in: self).y))

                event.setTranslation(.zero, in: self)

            }

            self.setNeedsDisplay()
        }

        public func setInitialContent(_ coreHandle: UnsafeMutableRawPointer?) {
            let metalLayer = UnsafeMutableRawPointer(
                Unmanaged.passUnretained(self.layer).toOpaque())
            self.wsHandle = init_ws(coreHandle, metalLayer, isDarkMode(), !isCompact())
            workspaceInput?.wsHandle = wsHandle
        }

        public func drawImmediately() {
            redrawTask?.cancel()
            redrawTask = nil

            ignoreSelectionUpdate = true
            ignoreTextUpdate = true

            self.isPaused = true
            self.enableSetNeedsDisplay = false

            self.mtkDelegate?.draw(in: self)

            ignoreSelectionUpdate = false
            ignoreTextUpdate = false
        }

        override public func traitCollectionDidChange(_ previousTraitCollection: UITraitCollection?)
        {
            setNeedsDisplay(self.frame)
        }

        public override func pressesBegan(_ presses: Set<UIPress>, with event: UIPressesEvent?) {
            let forward = handleKeyEvent(presses, with: event, pressBegan: true)
            if forward {
                super.pressesBegan(presses, with: event)
            }
        }

        public override func pressesEnded(_ presses: Set<UIPress>, with event: UIPressesEvent?) {
            let forward = handleKeyEvent(presses, with: event, pressBegan: false)
            if forward {
                super.pressesEnded(presses, with: event)
            }
        }

        /// Returns whether the event should be forwarded up the inheritance hierarchy
        func handleKeyEvent(_ presses: Set<UIPress>, with event: UIPressesEvent?, pressBegan: Bool)
            -> Bool
        {
            var forward = true

            for press in presses {
                guard let key = press.key else { continue }

                if workspaceOutput!.currentTab.isTextEdit()
                    && key.keyCode == .keyboardDeleteOrBackspace
                {
                    break
                }

                let shift = key.modifierFlags.contains(.shift)
                let ctrl = key.modifierFlags.contains(.control)
                let option = key.modifierFlags.contains(.alternate)
                let command = key.modifierFlags.contains(.command)

                if (command && key.keyCode == .keyboardW) || (shift && key.keyCode == .keyboardTab)
                {
                    forward = false
                }

                ios_key_event(
                    wsHandle, key.keyCode.rawValue, shift, ctrl, option, command, pressBegan)
                self.setNeedsDisplay(self.frame)
            }

            return forward
        }

        func importContent(_ importFormat: SupportedImportFormat, isPaste: Bool) {
            switch importFormat {
            case .url(let url):
                if url.pathExtension.lowercased() == "png" {
                    guard let data = try? Data(contentsOf: url) else {
                        return
                    }

                    sendImage(img: data, isPaste: isPaste)
                } else {
                    clipboard_send_file(wsHandle, url.path(percentEncoded: false), isPaste)
                }
            case .image(let image):
                if let img = image.pngData() ?? image.jpegData(compressionQuality: 1.0) {
                    sendImage(img: img, isPaste: isPaste)
                }
            case .text(let text):
                clipboard_paste(wsHandle, text)
            }
        }

        func sendImage(img: Data, isPaste: Bool) {
            let imgPtr = img.withUnsafeBytes {
                (pointer: UnsafeRawBufferPointer) -> UnsafePointer<UInt8> in
                return pointer.baseAddress!.assumingMemoryBound(to: UInt8.self)
            }

            clipboard_send_image(wsHandle, imgPtr, UInt(img.count), isPaste)
        }

        func isDarkMode() -> Bool {
            return traitCollection.userInterfaceStyle != .light
        }

        func isCompact() -> Bool {
            return traitCollection.horizontalSizeClass == .compact
        }

        deinit {
            deinit_editor(wsHandle)
        }

        func unimplemented() {
            print("unimplemented!")
            Thread.callStackSymbols.forEach { print($0) }
            //        exit(-69)
        }

        public override var canBecomeFocused: Bool {
            return true
        }
    }

    public enum SupportedImportFormat {
        case url(URL)
        case image(UIImage)
        case text(String)
    }

    // MARK: - LBTokenizer
    class LBTokenizer: NSObject, UITextInputTokenizer {
        let wsHandle: UnsafeMutableRawPointer?

        init(wsHandle: UnsafeMutableRawPointer?) {
            self.wsHandle = wsHandle
        }

        func isPosition(
            _ position: UITextPosition, atBoundary granularity: UITextGranularity,
            inDirection direction: UITextDirection
        ) -> Bool {
            guard let position = (position as? LBTextPos)?.c else {
                return false
            }
            let granularity = CTextGranularity(rawValue: UInt32(granularity.rawValue))
            let backwards = direction.rawValue == UITextStorageDirection.backward.rawValue
            return is_position_at_bound(wsHandle, position, granularity, backwards)
        }

        func isPosition(
            _ position: UITextPosition, withinTextUnit granularity: UITextGranularity,
            inDirection direction: UITextDirection
        ) -> Bool {
            guard let position = (position as? LBTextPos)?.c else {
                return false
            }
            let granularity = CTextGranularity(rawValue: UInt32(granularity.rawValue))
            let backwards = direction.rawValue == UITextStorageDirection.backward.rawValue
            return is_position_within_bound(wsHandle, position, granularity, backwards)
        }

        func position(
            from position: UITextPosition, toBoundary granularity: UITextGranularity,
            inDirection direction: UITextDirection
        ) -> UITextPosition? {
            guard let position = (position as? LBTextPos)?.c else {
                return nil
            }
            let granularity = CTextGranularity(rawValue: UInt32(granularity.rawValue))
            let backwards = direction.rawValue == UITextStorageDirection.backward.rawValue
            let result = bound_from_position(wsHandle, position, granularity, backwards)
            return LBTextPos(c: result)
        }

        func rangeEnclosingPosition(
            _ position: UITextPosition, with granularity: UITextGranularity,
            inDirection direction: UITextDirection
        ) -> UITextRange? {
            guard let position = (position as? LBTextPos)?.c else {
                return nil
            }
            let granularity = CTextGranularity(rawValue: UInt32(granularity.rawValue))
            let backwards = direction.rawValue == UITextStorageDirection.backward.rawValue
            let result = bound_at_position(wsHandle, position, granularity, backwards)

            if result.start.pos == result.end.pos {
                return nil
            }

            return LBTextRange(c: result)
        }
    }

    // MARK: - iOSUndoManager
    class iOSUndoManager: UndoManager {
        public var wsHandle: UnsafeMutableRawPointer? = nil

        weak var textWrapper: MdView? = nil
        weak var mtkView: iOSMTK? = nil
        weak var inputDelegate: UITextInputDelegate? = nil

        override var canUndo: Bool {
            can_undo(wsHandle)
        }

        override var canRedo: Bool {
            can_redo(wsHandle)
        }

        override func undo() {
            inputDelegate?.textWillChange(textWrapper)
            inputDelegate?.selectionWillChange(textWrapper)
            undo_redo(wsHandle, false)
            mtkView?.drawImmediately()
            inputDelegate?.selectionDidChange(textWrapper)
            inputDelegate?.textDidChange(textWrapper)
        }

        override func redo() {
            inputDelegate?.textWillChange(textWrapper)
            inputDelegate?.selectionWillChange(textWrapper)
            undo_redo(wsHandle, true)
            mtkView?.drawImmediately()
            inputDelegate?.selectionDidChange(textWrapper)
            inputDelegate?.textDidChange(textWrapper)
        }
    }

    // MARK: - FFI Wrappers
    class LBTextRange: UITextRange {
        let c: CTextRange

        init(c: CTextRange) {
            self.c = c
        }

        override var start: UITextPosition {
            return LBTextPos(c: c.start)
        }

        override var end: UITextPosition {
            return LBTextPos(c: c.end)
        }

        override var isEmpty: Bool {
            return c.start.pos >= c.end.pos
        }

        var length: Int {
            return Int(c.start.pos - c.end.pos)
        }
    }

    class LBTextPos: UITextPosition {
        let c: CTextPosition

        init(c: CTextPosition) {
            self.c = c
        }
    }

    class LBTextSelectionRect: UITextSelectionRect {
        let loc: Int
        let size: Int
        public let cRect: CRect

        init(cRect: CRect, loc: Int, size: Int) {
            self.cRect = cRect
            self.loc = loc
            self.size = size
        }

        override var writingDirection: NSWritingDirection {
            return .leftToRight
        }
        override var containsStart: Bool {
            return loc == 0
        }
        override var containsEnd: Bool {
            return loc == (size - 1)
        }
        override var isVertical: Bool {
            return false
        }

        override var rect: CGRect {
            return CGRect(
                x: cRect.min_x, y: cRect.min_y, width: cRect.max_x - cRect.min_x,
                height: cRect.max_y - cRect.min_y)
        }
    }

    // MARK: - WsTouchGestureRecognizer
    class WsTouchGestureRecognizer: UILongPressGestureRecognizer {
        var touch: UITouch?
        var event: UIEvent?

        var mtkView: iOSMTK?
        var toCancel: [UIGestureRecognizer]?

        convenience init() {
            self.init(target: nil, action: nil)
            minimumPressDuration = 0
            allowableMovement = .infinity
            name = "WsTouch"
        }

        public override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent) {
            super.touchesBegan(touches, with: event)
            if touch == nil {
                touch = touches.first
                self.event = event
            }

            guard let wsHandle = mtkView?.wsHandle else { return }

            for touch in touches {
                let location = touch.preciseLocation(in: self.mtkView)
                if will_consume_touch(wsHandle, Float(location.x), Float(location.y)) {
                    if self.state == .began || self.state == .changed {
                        for gr in toCancel ?? [] {
                            gr.state = .cancelled
                        }
                    }
                }
            }
        }

        public override func reset() {
            super.reset()
            touch = nil
            event = nil
        }
    }

    // MARK: - SvgPanGestureRecognizer
    /// Like UIPanGestureRecognizer, but cancels a WsTouchGestureRecognizer and times out
    class SvgPanGestureRecognizer: UIPanGestureRecognizer {
        var wsTouch: WsTouchGestureRecognizer?
        var start: Date?

        init(target: Any?, action: Selector, wsTouch: WsTouchGestureRecognizer?) {
            super.init(target: target, action: action)
            self.wsTouch = wsTouch
            name = "SvgPan"
        }

        public override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent) {
            super.touchesBegan(touches, with: event)

            if let firstTouchStart = start {
                if Date().timeIntervalSince(firstTouchStart) > 0.5 {
                    // fulfills "times out"
                    self.state = .cancelled
                }
            } else {
                start = Date()
            }
        }

        public override func touchesMoved(_ touches: Set<UITouch>, with event: UIEvent) {
            // fulfills "Like UIPanGestureRecognizer"
            super.touchesMoved(touches, with: event)

            // fulfills "cancels a WsTouchGestureRecognizer"
            if self.state == .began || self.state == .changed {
                wsTouch?.state = .cancelled
            }
        }

        public override func reset() {
            start = nil
            super.reset()
        }
    }

    // MARK: - SvgPinchGestureRecognizer
    /// Like UIPinchGestureRecognizer, but cancels a WsTouchGestureRecognizer and times out
    class SvgPinchGestureRecognizer: UIPinchGestureRecognizer {
        var wsTouch: WsTouchGestureRecognizer?
        var start: Date?

        init(target: Any?, action: Selector, wsTouch: WsTouchGestureRecognizer?) {
            super.init(target: target, action: action)
            self.wsTouch = wsTouch
            name = "SvgPinch"
        }

        public override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent) {
            super.touchesBegan(touches, with: event)

            if let firstTouchStart = start {
                if Date().timeIntervalSince(firstTouchStart) > 0.5 {
                    // fulfills "times out"
                    self.state = .cancelled
                }
            } else {
                start = Date()
            }
        }

        public override func touchesMoved(_ touches: Set<UITouch>, with event: UIEvent) {
            // fulfills "Like UIPinchGestureRecognizer"
            super.touchesMoved(touches, with: event)

            // fulfills "cancels a WsTouchGestureRecognizer"
            if self.state == .began || self.state == .changed {
                wsTouch?.state = .cancelled
            }
        }

        public override func reset() {
            super.reset()
            start = nil
        }
    }

#endif

public enum WorkspaceTab: Int {
    case Welcome = 0
    case Loading = 1
    case Image = 2
    case Markdown = 3
    case PlainText = 4
    case Pdf = 5
    case Svg = 6
    case Graph = 7
    case SpaceInspector = 8

    func viewWrapperId() -> Int {
        switch self {
        case .Welcome, .Pdf, .Loading, .SpaceInspector:
            1
        case .Svg, .Image, .Graph:
            2
        case .PlainText, .Markdown:
            3
        }
    }

    func isTextEdit() -> Bool {
        self == .Markdown || self == .PlainText
    }

    func isSvg() -> Bool {
        self == .Svg
    }
}
