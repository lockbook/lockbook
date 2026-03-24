#if os(iOS)
    import Bridge
    import GameController
    import MetalKit
    import MobileCoreServices
    import SwiftUI
    import UIKit
    import UniformTypeIdentifiers

    // MARK: - MdView

    public class MdView: UIView, UITextInput {
        public static let TOOL_BAR_HEIGHT: CGFloat = 42
        public static let FLOATING_CURSOR_OFFSET_HEIGHT: CGFloat = 0.6

        let mtkView: iOSMTK
        var wsHandle: UnsafeMutableRawPointer? {
            mtkView.wsHandle
        }

        // text input
        let textInteraction = UITextInteraction(for: .editable)
        public var inputDelegate: UITextInputDelegate?
        public lazy var tokenizer: UITextInputTokenizer = LBTokenizer(wsHandle: self.wsHandle)

        /// pan (iPad trackpad support)
        let panRecognizer = UIPanGestureRecognizer()

        // undo/redo
        let undo = iOSUndoManager()
        override public var undoManager: UndoManager? {
            undo
        }

        /// drop
        var dropDelegate: UIDropInteractionDelegate?

        /// ?
        let currentHeaderSize: Double

        // interactive refinement (floating cursor)
        var interactiveRefinementInProgress = false
        var lastFloatingCursorRect: CGRect?
        var floatingCursor: UIView = .init()
        var floatingCursorWidth = 1.0
        var floatingCursorNewStartX = 0.0
        var floatingCursorNewEndX = 0.0
        var floatingCursorNewStartY = 0.0
        var floatingCursorNewEndY = 0.0
        var autoScroll: Timer?

        /// range adjustment (selection handles)
        var rangeAdjustmentInProgress = false

        init(mtkView: iOSMTK, headerSize: Double) {
            self.mtkView = mtkView
            currentHeaderSize = headerSize

            super.init(frame: .infinite)

            clipsToBounds = true
            isUserInteractionEnabled = true

            // text input
            textInteraction.textInput = self
            addInteraction(textInteraction)

            for gestureRecognizer in gestureRecognizers ?? [] {
                // receive touch events immediately even if they are part of any recognized gestures
                // this supports checkboxes and other interactive markdown elements in the text area (crudely)
                gestureRecognizer.delaysTouchesBegan = false
                gestureRecognizer.delaysTouchesEnded = false

                switch gestureRecognizer.name {
                case "UITextInteractionNameInteractiveRefinement":
                    // send interactive refinements to our handler
                    // this is the intended way to support a floating cursor
                    gestureRecognizer.addTarget(
                        self, action: #selector(handleInteractiveRefinement(_:))
                    )

                    // workspace gets priority on single taps; see checkCancelCursorPlacement()
                    gestureRecognizer.cancelsTouchesInView = false
                case "UITextInteractionNameSingleTap":
                    // workspace gets priority on single taps; see checkCancelCursorPlacement()
                    gestureRecognizer.cancelsTouchesInView = false
                case "UITextInteractionNameTapAndAHalf":
                    break
                case "UITextInteractionNameRangeAdjustment":
                    // send range adjustments to our handler
                    // this supports automatic scrolling when dragging selection handles
                    gestureRecognizer.addTarget(
                        self, action: #selector(handleRangeAdjustment(_:))
                    )
                case "UITextInteractionNameLinkTap":
                    break
                default:
                    break
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
            addGestureRecognizer(panRecognizer)

            // drop
            dropDelegate = MdDropDelegate(
                mtkView: mtkView, textDelegate: inputDelegate!, textInput: self
            )
            let dropInteraction = UIDropInteraction(delegate: dropDelegate!)
            addInteraction(dropInteraction)

            // undo/redo
            undo.textWrapper = self
            undo.wsHandle = wsHandle
            undo.mtkView = mtkView
            undo.inputDelegate = inputDelegate

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
                scrollTo(location.y)
            case .ended, .cancelled, .failed:
                rangeAdjustmentInProgress = false
            default:
                break
            }
        }

        @available(*, unavailable)
        required init(coder _: NSCoder) {
            fatalError("init(coder:) has not been implemented")
        }

        override public func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent?) {
            mtkView.touchesBegan(touches, with: event)
            checkCancelCursorPlacement(touches)
        }

        override public func touchesMoved(_ touches: Set<UITouch>, with event: UIEvent?) {
            mtkView.touchesMoved(touches, with: event)
            checkCancelCursorPlacement(touches)
        }

        override public func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent?) {
            mtkView.touchesEnded(touches, with: event)
            checkCancelCursorPlacement(touches)
        }

        override public func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent?) {
            mtkView.touchesCancelled(touches, with: event)
        }

        private func checkCancelCursorPlacement(_ touches: Set<UITouch>) {
            // consumed workspace touches (e.g. tapping to stop a kinetic scroll or toggle a checkbox) preclude cursor placement
            for touch in touches {
                let location = touch.location(in: mtkView)
                if will_consume_touch(wsHandle, Float(location.x), Float(location.y)) {
                    for gestureRecognizer in gestureRecognizers ?? [] {
                        switch gestureRecognizer.name {
                        case "UITextInteractionNameSingleTap":
                            gestureRecognizer.state = .failed
                        case "UITextInteractionNameInteractiveRefinement":
                            gestureRecognizer.state = .failed
                        default:
                            break
                        }
                    }
                }
            }
        }

        public func beginFloatingCursor(at point: CGPoint) {
            tintColor = traitCollection.userInterfaceStyle == .light ? .lightGray : .gray
            floatingCursorNewEndX = bounds.size.width
            floatingCursorNewEndY = bounds.size.height

            setFloatingCursorLoc(point: point, animate: false)
            bringSubviewToFront(floatingCursor)
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
                                height: cursorRect.height + Self.FLOATING_CURSOR_OFFSET_HEIGHT
                            )
                        }
                    },
                    completion: { [weak self] _ in
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
                    }
                )
            }
        }

        func setFloatingCursorLoc(point: CGPoint, animate: Bool) {
            let pos = closestPosition(to: point)
            let cursorRect = caretRect(for: pos!)

            lastFloatingCursorRect = cursorRect

            let x = point.x - floatingCursorNewStartX
            let y = point.y - floatingCursorNewStartY

            scrollTo(y)

            if animate {
                UIView.animate(
                    withDuration: 0.15,
                    animations: { [weak self] in
                        if let textWrapper = self {
                            textWrapper.floatingCursor.frame = CGRect(
                                x: x, y: y - (cursorRect.height / 2),
                                width: textWrapper.floatingCursorWidth,
                                height: cursorRect.height + Self.FLOATING_CURSOR_OFFSET_HEIGHT
                            )
                        }
                    }
                )
            } else {
                floatingCursor.frame = CGRect(
                    x: x, y: y - (cursorRect.height / 2), width: floatingCursorWidth,
                    height: cursorRect.height + Self.FLOATING_CURSOR_OFFSET_HEIGHT
                )
            }
        }

        func scrollTo(_ y: CGFloat) {
            let scrollUp = y >= bounds.height - 20
            let scrollDown = y <= 20

            if scrollUp || scrollDown, autoScroll == nil {
                autoScroll = Timer.scheduledTimer(withTimeInterval: 0.016, repeats: true) {
                    [self] timer in
                    if floatingCursor.isHidden, !rangeAdjustmentInProgress {
                        timer.invalidate()
                    }

                    mouse_moved(wsHandle, Float(bounds.width / 2), Float(bounds.height / 2))
                    scroll_wheel(wsHandle, 0, scrollUp ? -20 : 20, false, false, false, false)
                    mouse_gone(wsHandle)

                    mtkView.drawImmediately()
                }
            } else if let autoScroll,
                      !scrollUp, !scrollDown
            {
                autoScroll.invalidate()
                self.autoScroll = nil
            }
        }

        override public func canPerformAction(_ action: Selector, withSender sender: Any?) -> Bool {
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
                    end: CTextPosition(none: false, pos: UInt(selectedRange.upperBound))
                ),
                markedText
            )
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

            return if left < right {
                ComparisonResult.orderedAscending
            } else if left == right {
                ComparisonResult.orderedSame
            } else {
                ComparisonResult.orderedDescending
            }
        }

        public func offset(from: UITextPosition, to toPosition: UITextPosition) -> Int {
            guard let left = (from as? LBTextPos)?.c.pos,
                  let right = (toPosition as? LBTextPos)?.c.pos
            else {
                return 0
            }

            return Int(right) - Int(left)
        }

        public func position(within _: UITextRange, farthestIn _: UITextLayoutDirection)
            -> UITextPosition?
        {
            unimplemented()
            return nil
        }

        public func characterRange(
            byExtending _: UITextPosition, in _: UITextLayoutDirection
        ) -> UITextRange? {
            unimplemented()
            return nil
        }

        public func baseWritingDirection(
            for _: UITextPosition, in _: UITextStorageDirection
        ) -> NSWritingDirection {
            NSWritingDirection.leftToRight
        }

        public func setBaseWritingDirection(
            _ writingDirection: NSWritingDirection, for _: UITextRange
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
                width: result.max_x - result.min_x, height: result.max_y - result.min_y
            )
        }

        public func caretRect(for position: UITextPosition) -> CGRect {
            let position = (position as! LBTextPos).c
            let result = cursor_rect_at_position(wsHandle, position)
            return CGRect(
                x: result.min_x, y: result.min_y - mtkView.docHeaderSize, width: 1,
                height: result.max_y - result.min_y
            )
        }

        public func selectionRects(for range: UITextRange) -> [UITextSelectionRect] {
            let range = (range as! LBTextRange).c
            let result = selection_rects(wsHandle, range)

            let buffer = Array(UnsafeBufferPointer(start: result.rects, count: Int(result.size)))

            free_selection_rects(result)

            return buffer.enumerated().map { index, rect in
                let new_rect = CRect(
                    min_x: rect.min_x, min_y: rect.min_y - mtkView.docHeaderSize, max_x: rect.max_x,
                    max_y: rect.max_y - mtkView.docHeaderSize
                )

                return LBTextSelectionRect(cRect: new_rect, loc: index, size: buffer.count)
            }
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

        public func closestPosition(to _: CGPoint, within _: UITextRange) -> UITextPosition? {
            unimplemented()
            return nil
        }

        public func characterRange(at _: CGPoint) -> UITextRange? {
            unimplemented()
            return nil
        }

        public var hasText: Bool {
            has_text(wsHandle)
        }

        public var isEditable: Bool {
            is_current_tab_editable(wsHandle)
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

        override public func cut(_: Any?) {
            guard let range = (markedTextRange ?? selectedTextRange) as? LBTextRange,
                  !range.isEmpty
            else {
                return
            }

            inputDelegate?.textWillChange(self)
            inputDelegate?.selectionWillChange(self)
            clipboard_cut(wsHandle)
            mtkView.drawImmediately()
            inputDelegate?.selectionDidChange(self)
            inputDelegate?.textDidChange(self)
        }

        override public func copy(_: Any?) {
            clipboard_copy(wsHandle)
            setNeedsDisplay(frame)
        }

        override public func paste(_: Any?) {
            if let image = UIPasteboard.general.image {
                importContent(.image(image), isPaste: true)
            } else if let string = UIPasteboard.general.string {
                importContent(.text(string), isPaste: true)
            }
        }

        override public func selectAll(_: Any?) {
            if !hasText {
                return
            }

            inputDelegate?.selectionWillChange(self)
            select_all(wsHandle)
            mtkView.drawImmediately()
            inputDelegate?.selectionDidChange(self)
        }

        func getText() -> String {
            let result = get_text(wsHandle)
            let str = String(cString: result!)
            free_text(UnsafeMutablePointer(mutating: result))
            return str
        }

        override public var canBecomeFirstResponder: Bool {
            true
        }

        override public var keyCommands: [UIKeyCommand]? {
            let deleteWord = UIKeyCommand(
                input: UIKeyCommand.inputDelete, modifierFlags: [.alternate],
                action: #selector(deleteWord)
            )

            deleteWord.wantsPriorityOverSystemBehavior = true

            return [
                deleteWord,
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

        override public func pressesBegan(_ presses: Set<UIPress>, with event: UIPressesEvent?) {
            let forward = mtkView.handleKeyEvent(presses, with: event, pressBegan: true)
            if forward {
                super.pressesBegan(presses, with: event)
            }
        }

        override public func pressesEnded(_ presses: Set<UIPress>, with event: UIPressesEvent?) {
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
            _: UIDropInteraction, canHandle session: UIDropSession
        ) -> Bool {
            guard session.items.count == 1 else { return false }

            return session.hasItemsConforming(toTypeIdentifiers: [
                UTType.image.identifier, UTType.fileURL.identifier, UTType.text.identifier,
            ])
        }

        public func dropInteraction(
            _: UIDropInteraction, sessionDidUpdate _: UIDropSession
        ) -> UIDropProposal {
            UIDropProposal(operation: .copy)
        }

        public func dropInteraction(
            _: UIDropInteraction, performDrop session: UIDropSession
        ) {
            guard let mtkView else { return }
            guard let textDelegate else { return }

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

            if session.hasItemsConforming(toTypeIdentifiers: [UTType.fileURL.identifier as String]) {
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
        let mtkView: iOSMTK
        var wsHandle: UnsafeMutableRawPointer? {
            mtkView.wsHandle
        }

        /// pointer
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

        /// ?
        let currentHeaderSize: Double

        init(mtkView: iOSMTK, headerSize: Double) {
            self.mtkView = mtkView
            currentHeaderSize = headerSize

            super.init(frame: .infinite)

            isMultipleTouchEnabled = true

            // pointer
            let pointerInteraction = UIPointerInteraction(delegate: mtkView.pointerDelegate)

            addInteraction(pointerInteraction)

            self.pointerInteraction = pointerInteraction

            // pencil
            let pencilDelegate = SvgPencilDelegate(mtkView: mtkView)
            let pencilInteraction = UIPencilInteraction()

            pencilInteraction.delegate = pencilDelegate
            addInteraction(pencilInteraction)

            self.pencilDelegate = pencilDelegate
            self.pencilInteraction = pencilInteraction

            // gestures
            gestureDelegate = SvgGestureDelegate()

            // gestures: tap
            let tap = UITapGestureRecognizer(target: self, action: #selector(handleTap(_:)))
            tap.allowedTouchTypes = [
                NSNumber(value: UITouch.TouchType.direct.rawValue),
                NSNumber(value: UITouch.TouchType.indirect.rawValue),
                // NSNumber(value: UITouch.TouchType.pencil.rawValue),
                NSNumber(value: UITouch.TouchType.indirectPointer.rawValue),
            ]
            tap.numberOfTouchesRequired = 1
            tap.cancelsTouchesInView = false

            addGestureRecognizer(tap)

            tapRecognizer = tap

            // gestures: pan
            let pan = UIPanGestureRecognizer(target: self, action: #selector(handlePan(_:)))
            pan.allowedTouchTypes = [
                NSNumber(value: UITouch.TouchType.direct.rawValue),
                NSNumber(value: UITouch.TouchType.indirect.rawValue),
                // NSNumber(value: UITouch.TouchType.pencil.rawValue),
                NSNumber(value: UITouch.TouchType.indirectPointer.rawValue),
            ]
            pan.allowedScrollTypesMask = .all
            if UIPencilInteraction.prefersPencilOnlyDrawing { // TODO: update when prefersPencilOnlyDrawing changes
                pan.minimumNumberOfTouches = 1
            } else {
                pan.minimumNumberOfTouches = 2
            }
            pan.cancelsTouchesInView = false

            pan.delegate = gestureDelegate
            addGestureRecognizer(pan)

            panRecognizer = pan

            // gestures: pinch
            let pinch = UIPinchGestureRecognizer(
                target: self, action: #selector(handlePinch(_:))
            )
            pinch.cancelsTouchesInView = false

            pinch.delegate = gestureDelegate
            addGestureRecognizer(pinch)

            pinchRecognizer = pinch

            // menu
            let menuDelegate = SvgMenuDelegate()
            let menuInteraction = UIEditMenuInteraction(delegate: menuDelegate)

            addInteraction(menuInteraction)

            self.menuDelegate = menuDelegate
            self.menuInteraction = menuInteraction
        }

        @objc private func handleTap(_ gesture: UITapGestureRecognizer) {
            guard let menuInteraction else { return }

            if gesture.state != .ended { return }

            if mtkView.kineticTimer != nil {
                mtkView.kineticTimer?.invalidate()
                mtkView.kineticTimer = nil
                return
            }

            // Check if we have any valid actions before presenting
            let location = gesture.location(in: mtkView)
            if will_consume_touch(wsHandle, Float(location.x), Float(location.y))
                || (!UIPasteboard.general.hasStrings && !UIPasteboard.general.hasImages)
            {
                return
            }

            let config = UIEditMenuConfiguration(identifier: nil, sourcePoint: location)
            menuInteraction.presentEditMenu(with: config)
        }

        override public func canPerformAction(_ action: Selector, withSender _: Any?) -> Bool {
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

            if mtkView.kineticTimer != nil {
                mtkView.kineticTimer?.invalidate()
                mtkView.kineticTimer = nil
            }

            let scale = event.scale
            let pinchCenter = event.location(in: mtkView)

            if event.state == .changed {
                let zoomDelta = Float(scale)

                zoom(wsHandle, zoomDelta)

                let viewCenter = CGPoint(x: mtkView.bounds.midX, y: mtkView.bounds.midY)
                let offsetX = pinchCenter.x - viewCenter.x
                let offsetY = pinchCenter.y - viewCenter.y

                let panX = offsetX * (scale - 1.0)
                let panY = offsetY * (scale - 1.0)

                pan(wsHandle, Float(panX), Float(panY))

                event.scale = 1.0
            }
        }

        override public func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent?) {
            set_pencil_only_drawing(wsHandle, UIPencilInteraction.prefersPencilOnlyDrawing)

            // let's cancel the kinetic pan. don't nullify it so that the tap handler
            // will know not to show the edit menu
            if mtkView.kineticTimer != nil {
                mtkView.kineticTimer?.invalidate()
            }

            mtkView.touchesBegan(touches, with: event)
        }

        override public func touchesMoved(_ touches: Set<UITouch>, with event: UIEvent?) {
            mtkView.touchesMoved(touches, with: event)
        }

        override public func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent?) {
            mtkView.touchesEnded(touches, with: event)
        }

        override public func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent?) {
            mtkView.touchesCancelled(touches, with: event)
        }

        override public func paste(_: Any?) {
            if let image = UIPasteboard.general.image {
                mtkView.importContent(.image(image), isPaste: true)
            } else if let string = UIPasteboard.general.string {
                mtkView.importContent(.text(string), isPaste: true)
            }
        }

        @available(*, unavailable)
        required init(coder _: NSCoder) {
            fatalError("init(coder:) has not been implemented")
        }
    }

    // MARK: - SvgGestureDelegate

    public class SvgGestureDelegate: NSObject, UIGestureRecognizerDelegate {
        public func gestureRecognizer(
            _ gestureRecognizer: UIGestureRecognizer,
            shouldRecognizeSimultaneouslyWith otherGestureRecognizer: UIGestureRecognizer
        ) -> Bool {
            // Allow pinch and pan to work together
            if (gestureRecognizer is UIPinchGestureRecognizer
                && otherGestureRecognizer is UIPanGestureRecognizer)
                || (gestureRecognizer is UIPanGestureRecognizer
                    && otherGestureRecognizer is UIPinchGestureRecognizer)
            {
                return true
            }

            return false
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
            _: UIPencilInteraction,
            didReceiveSqueeze squeeze: UIPencilInteraction.Squeeze
        ) {
            guard let mtkView else { return }

            if squeeze.phase == .ended {
                show_tool_popover_at_cursor(mtkView.wsHandle)
            }
        }

        public func pencilInteractionDidTap(_: UIPencilInteraction) {
            guard let mtkView else { return }

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

        public func mtkView(_: MTKView, drawableSizeWillChange size: CGSize) {
            guard let mtkView else { return }
            let wsHandle = mtkView.wsHandle

            resize_editor(
                wsHandle, Float(size.width), Float(size.height), Float(scale())
            )
            mtkView.setNeedsDisplay()
        }

        public func draw(in _: MTKView) {
            guard let mtkView else { return }
            let wsHandle = mtkView.wsHandle

            if mtkView.tabSwitchTask != nil {
                mtkView.tabSwitchTask!()
                mtkView.tabSwitchTask = nil
            }

            dark_mode(wsHandle, mtkView.isDarkMode())
            show_hide_tabs(wsHandle, !mtkView.isCompact())

            set_scale(wsHandle, Float(scale()))
            let keyboardTop = mtkView.keyboardLayoutGuide.layoutFrame.minY
            let overlap = max(0, mtkView.bounds.maxY - keyboardTop)
            set_ws_inset(wsHandle, Float(overlap * scale()))

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

            if currentTab == .Welcome, mtkView.currentOpenDoc != nil {
                mtkView.currentOpenDoc = nil
                mtkView.workspaceOutput?.openDoc = nil
            }

            if let currentWrapper = mtkView.currentWrapper as? MdView,
               currentTab == .Markdown
            {
                if output.has_virtual_keyboard_shown, !output.virtual_keyboard_shown,
                   currentWrapper.floatingCursor.isHidden
                {
                    UIApplication.shared.sendAction(
                        #selector(UIResponder.resignFirstResponder), to: nil, from: nil, for: nil
                    )
                }

                if output.scroll_updated {
                    mtkView.onSelectionChanged?()
                }

                if output.text_updated, !mtkView.ignoreTextUpdate {
                    mtkView.onTextChanged?()
                }

                if output.selection_updated, !mtkView.ignoreSelectionUpdate {
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

            //      FIXME: Can we just do this in rust?
            let newFile = UUID(uuid: output.doc_created._0)
            if !newFile.isNil() {
                mtkView.workspaceInput?.openFile(id: newFile)
            }

            if output.urls_opened.size > 0 {
                var urls: [URL] = []
                for i in 0..<Int(output.urls_opened.size) {
                    if let ptr = output.urls_opened.urls[i],
                       let url = URL(string: textFromPtr(s: ptr)),
                       UIApplication.shared.canOpenURL(url)
                    {
                        urls.append(url)
                    }
                }
                mtkView.workspaceOutput?.urlsOpened = urls
                free_urls(output.urls_opened)
            }

            if output.open_camera {
                mtkView.workspaceOutput?.openCamera = true
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
                    Int(truncatingIfNeeded: min(500, redrawIn))
                )

                let newRedrawTask = DispatchWorkItem {
                    mtkView.drawImmediately()
                }
                DispatchQueue.main.asyncAfter(
                    deadline: .now() + redrawInInterval, execute: newRedrawTask
                )
                mtkView.redrawTask = newRedrawTask
            }

            mtkView.enableSetNeedsDisplay = mtkView.isPaused
        }

        func scale() -> CGFloat {
            mtkView?.contentScaleFactor ?? CGFloat(1.0)
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
            guard let mtkView else { return defaultRegion }
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
            _: UIPointerInteraction, willEnter _: UIPointerRegion,
            animator _: any UIPointerInteractionAnimating
        ) {
            guard let mtkView else { return }

            mtkView.cursorTracked = true
        }

        public func pointerInteraction(
            _: UIPointerInteraction, willExit _: UIPointerRegion,
            animator _: any UIPointerInteractionAnimating
        ) {
            guard let mtkView else { return }

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
        weak var currentWrapper: UIView?

        // pointer
        var pointerInteraction: UIPointerInteraction?
        var pointerDelegate: UIPointerInteractionDelegate?

        /// gestures
        var panRecognizer: UIPanGestureRecognizer?

        // mtk
        var mtkDelegate: iOSMTKViewDelegate?
        var redrawTask: DispatchWorkItem?

        // workspace
        var workspaceOutput: WorkspaceOutputState?
        var workspaceInput: WorkspaceInputState?
        var currentOpenDoc: UUID? // TODO: duplicated in ws output
        var currentSelectedFolder: UUID? // duplicated in ws output

        // view hierarchy management
        var tabSwitchTask: (() -> Void)? // facilitates switching wrapper views in response to tab change
        var onSelectionChanged: (() -> Void)? // only populated when wrapper is markdown
        var onTextChanged: (() -> Void)? // also only populated when wrapper is markdown
        var ignoreSelectionUpdate = false // don't invoke corresponding handler when drawing immediately
        var ignoreTextUpdate = false // also don't invoke corresponding handler when drawing immediately
        var docHeaderSize: Double {
            !isCompact() ? iOSMTK.TAB_BAR_HEIGHT : 0
        }

        // kinetic scroll
        var cursorTracked = false
        var scrollSensitivity = 50.0
        var scrollId = 0
        var kineticTimer: Timer?

        override init(frame frameRect: CGRect, device: MTLDevice?) {
            super.init(frame: frameRect, device: device)

            // pointer
            let pointerDelegate = iOSPointerDelegate(mtkView: self)
            let pointer = UIPointerInteraction(delegate: pointerDelegate)

            addInteraction(pointer)

            self.pointerDelegate = pointerDelegate
            pointerInteraction = pointer

            // gestures
            let pan = UIPanGestureRecognizer(
                target: self, action: #selector(handleTrackpadScroll(_:))
            )
            pan.allowedScrollTypesMask = .all
            pan.maximumNumberOfTouches = 0

            addGestureRecognizer(pan)
            panRecognizer = pan

            // mtk
            mtkDelegate = iOSMTKViewDelegate(mtkView: self)

            isPaused = false
            enableSetNeedsDisplay = false
            delegate = mtkDelegate
            preferredFramesPerSecond = 120
            isUserInteractionEnabled = true
        }

        @available(*, unavailable)
        required init(coder _: NSCoder) {
            fatalError("init(coder:) has not been implemented")
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

                    if abs(velocity.x) < 0.1, abs(velocity.y) < 0.1 {
                        timer.invalidate()
                        return
                    }

                    if !cursorTracked {
                        mouse_moved(wsHandle, Float(bounds.width / 2), Float(bounds.height / 2))
                    }
                    scroll_wheel(
                        wsHandle, Float(velocity.x), Float(velocity.y), false, false, false,
                        false
                    )
                    if !cursorTracked {
                        mouse_gone(wsHandle)
                    }

                    setNeedsDisplay()
                }
            } else {
                if !cursorTracked {
                    mouse_moved(wsHandle, Float(bounds.width / 2), Float(bounds.height / 2))
                }
                scroll_wheel(
                    wsHandle, Float(velocity.x), Float(velocity.y), false, false, false, false
                )
                if !cursorTracked {
                    mouse_gone(wsHandle)
                }
            }

            setNeedsDisplay()
        }

        /// used in canvas
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
                touches_ended(wsHandle, UInt64.random(in: UInt64.min ... UInt64.max), 0.0, 0.0, 0.0)

                kineticTimer = Timer.scheduledTimer(withTimeInterval: 0.016, repeats: true) {
                    [weak self] timer in
                    guard let self else {
                        timer.invalidate()
                        self?.kineticTimer = nil
                        return
                    }

                    velocity.x *= Self.POINTER_DECELERATION_RATE
                    velocity.y *= Self.POINTER_DECELERATION_RATE

                    if abs(velocity.x) < 0.1, abs(velocity.y) < 0.1 {
                        timer.invalidate()
                        kineticTimer = nil
                        return
                    }

                    pan(wsHandle, Float(velocity.x), Float(velocity.y))
                    mouse_gone(wsHandle)

                    setNeedsDisplay()
                }
            } else {
                let translation = event.translation(in: self)

                pan(wsHandle, Float(translation.x), Float(translation.y))
                mouse_moved(
                    wsHandle, Float(event.location(in: self).x), Float(event.location(in: self).y)
                )

                event.setTranslation(.zero, in: self)
            }

            setNeedsDisplay()
        }

        public func setInitialContent(_ coreHandle: UnsafeMutableRawPointer?) {
            let metalLayer = UnsafeMutableRawPointer(
                Unmanaged.passUnretained(layer).toOpaque()
            )
            wsHandle = init_ws(coreHandle, metalLayer, isDarkMode(), !isCompact())
            workspaceInput?.wsHandle = wsHandle
        }

        public func drawImmediately() {
            redrawTask?.cancel()
            redrawTask = nil

            ignoreSelectionUpdate = true
            ignoreTextUpdate = true

            isPaused = true
            enableSetNeedsDisplay = false

            mtkDelegate?.draw(in: self)

            ignoreSelectionUpdate = false
            ignoreTextUpdate = false
        }

        override public func traitCollectionDidChange(_: UITraitCollection?) {
            setNeedsDisplay(frame)
        }

        override public func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent?) {
            for touch in touches {
                let point = Unmanaged.passUnretained(touch).toOpaque()
                let value = UInt64(UInt(bitPattern: point))

                for touch in event!.coalescedTouches(for: touch)! {
                    let location = touch.preciseLocation(in: self)
                    let force = touch.force != 0 ? touch.force / touch.maximumPossibleForce : 0
                    touches_began(
                        wsHandle, value, Float(location.x), Float(location.y), Float(force)
                    )
                }
            }

            setNeedsDisplay(frame)
        }

        override public func touchesMoved(_ touches: Set<UITouch>, with event: UIEvent?) {
            for touch in touches {
                let point = Unmanaged.passUnretained(touch).toOpaque()
                let value = UInt64(UInt(bitPattern: point))

                let location = touch.preciseLocation(in: self)
                let force = touch.force != 0 ? touch.force / touch.maximumPossibleForce : 0

                for touch in event!.predictedTouches(for: touch)! {
                    let location = touch.preciseLocation(in: self)
                    let force = touch.force != 0 ? touch.force / touch.maximumPossibleForce : 0
                    touches_predicted(
                        wsHandle, value, Float(location.x), Float(location.y), Float(force)
                    )
                }

                touches_moved(wsHandle, value, Float(location.x), Float(location.y), Float(force))
            }

            setNeedsDisplay(frame)
        }

        override public func touchesEnded(_ touches: Set<UITouch>, with _: UIEvent?) {
            for touch in touches {
                let point = Unmanaged.passUnretained(touch).toOpaque()
                let value = UInt64(UInt(bitPattern: point))

                let location = touch.preciseLocation(in: self)
                let force = touch.force != 0 ? touch.force / touch.maximumPossibleForce : 0
                touches_ended(wsHandle, value, Float(location.x), Float(location.y), Float(force))
            }

            setNeedsDisplay(frame)
        }

        override public func touchesCancelled(_ touches: Set<UITouch>, with _: UIEvent?) {
            for touch in touches {
                let point = Unmanaged.passUnretained(touch).toOpaque()
                let value = UInt64(UInt(bitPattern: point))

                let location = touch.preciseLocation(in: self)
                let force = touch.force != 0 ? touch.force / touch.maximumPossibleForce : 0

                touches_cancelled(
                    wsHandle, value, Float(location.x), Float(location.y), Float(force)
                )
            }

            setNeedsDisplay(frame)
        }

        override public func pressesBegan(_ presses: Set<UIPress>, with event: UIPressesEvent?) {
            let forward = handleKeyEvent(presses, with: event, pressBegan: true)
            if forward {
                super.pressesBegan(presses, with: event)
            }
        }

        override public func pressesEnded(_ presses: Set<UIPress>, with event: UIPressesEvent?) {
            let forward = handleKeyEvent(presses, with: event, pressBegan: false)
            if forward {
                super.pressesEnded(presses, with: event)
            }
        }

        /// Returns whether the event should be forwarded up the inheritance hierarchy
        func handleKeyEvent(_ presses: Set<UIPress>, with _: UIPressesEvent?, pressBegan: Bool)
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

                if (command && key.keyCode == .keyboardW) || (shift && key.keyCode == .keyboardTab) {
                    forward = false
                }

                ios_key_event(
                    wsHandle, key.keyCode.rawValue, shift, ctrl, option, command, pressBegan
                )
                setNeedsDisplay(frame)
            }

            return forward
        }

        func importContent(_ importFormat: SupportedImportFormat, isPaste: Bool) {
            switch importFormat {
            case let .url(url):
                if url.pathExtension.lowercased() == "png" {
                    guard let data = try? Data(contentsOf: url) else {
                        return
                    }

                    workspaceInput?.pasteImage(data: data, isPaste: isPaste)
                } else {
                    clipboard_send_file(wsHandle, url.path(percentEncoded: false), isPaste)
                }
            case let .image(image):
                let image = image.normalizedImage()
                if let data = image.pngData() ?? image.jpegData(compressionQuality: 1.0) {
                    workspaceInput?.pasteImage(data: data, isPaste: isPaste)
                }
            case let .text(text):
                clipboard_paste(wsHandle, text)
            }
        }

        func isDarkMode() -> Bool {
            traitCollection.userInterfaceStyle != .light
        }

        func isCompact() -> Bool {
            traitCollection.horizontalSizeClass == .compact
        }

        deinit {
            deinit_editor(wsHandle)
        }

        func unimplemented() {
            print("unimplemented!")
            Thread.callStackSymbols.forEach { print($0) }
            //        exit(-69)
        }

        override public var canBecomeFocused: Bool {
            true
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
        var wsHandle: UnsafeMutableRawPointer?

        weak var textWrapper: MdView?
        weak var mtkView: iOSMTK?
        weak var inputDelegate: UITextInputDelegate?

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
            LBTextPos(c: c.start)
        }

        override var end: UITextPosition {
            LBTextPos(c: c.end)
        }

        override var isEmpty: Bool {
            c.start.pos >= c.end.pos
        }

        var length: Int {
            Int(c.start.pos - c.end.pos)
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
        let cRect: CRect

        init(cRect: CRect, loc: Int, size: Int) {
            self.cRect = cRect
            self.loc = loc
            self.size = size
        }

        override var writingDirection: NSWritingDirection {
            .leftToRight
        }

        override var containsStart: Bool {
            loc == 0
        }

        override var containsEnd: Bool {
            loc == (size - 1)
        }

        override var isVertical: Bool {
            false
        }

        override var rect: CGRect {
            CGRect(
                x: cRect.min_x, y: cRect.min_y, width: cRect.max_x - cRect.min_x,
                height: cRect.max_y - cRect.min_y
            )
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
