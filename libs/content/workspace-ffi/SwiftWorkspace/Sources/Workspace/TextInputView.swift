#if os(iOS)
    import Bridge
    import GameController
    import UIKit
    import UniformTypeIdentifiers

    /// The system text input over the Metal editor. UIKit owns the caret,
    /// handles, loupe, menus, keyboard, and first responder. The editor owns
    /// the text, its layout, and the page scroll, which the page's pan feeds.
    ///
    /// Every touch is decided by a gesture recognizer so UIKit can arbitrate:
    /// no raw touches reach the editor from here. Touches on the editor's
    /// touch targets are declined at hit-test time and go to the page's tap
    /// layer, so UIKit's text gestures never see them.
    final class TextInputView: UIView, UITextInput {
        unowned let mtkView: iOSMTK
        /// Weak: UIKit may keep this view after its page is gone.
        weak var page: TextPage?
        var wsHandle: UnsafeMutableRawPointer? { mtkView.wsHandle }

        private let textInteraction = UITextInteraction(for: .editable)
        private let history = EditorUndoManager()
        /// A UIKit tap, loupe drag, or handle drag is writing the selection.
        private var textInteractionActive = false

        weak var inputDelegate: UITextInputDelegate?
        lazy var tokenizer: UITextInputTokenizer = EditorTokenizer(textInput: self)
        var markedTextStyle: [NSAttributedString.Key: Any]?
        /// The marked range as UIKit set it.
        private var marked: (lo: Int, hi: Int)?

        init(mtkView: iOSMTK, page: TextPage) {
            self.mtkView = mtkView
            self.page = page
            super.init(frame: .zero)
            backgroundColor = .clear
            clipsToBounds = true

            history.view = self
            textInteraction.textInput = self
            textInteraction.delegate = self
            addInteraction(textInteraction)
            addInteraction(UIDropInteraction(delegate: self))
        }

        /// The software keyboard is up: first responder without a hardware keyboard.
        var isSoftwareKeyboardUp: Bool {
            isFirstResponder && GCKeyboard.coalesced == nil
        }

        @available(*, unavailable)
        required init?(coder _: NSCoder) {
            fatalError("init(coder:) has not been implemented")
        }

        override var canBecomeFirstResponder: Bool { true }

        override func didMoveToWindow() {
            super.didMoveToWindow()
            focusForHardwareKeyboard()
        }

        /// With a hardware keyboard the editor keeps focus, so opening or
        /// switching notes leaves you typing.
        @objc func focusForHardwareKeyboard() {
            guard GCKeyboard.coalesced != nil, window != nil, !isFirstResponder else { return }
            becomeFirstResponder()
        }

        @discardableResult
        override func becomeFirstResponder() -> Bool {
            let result = super.becomeFirstResponder()
            // Focus from code leaves the caret hidden until a tap (FB12622609);
            // Runestone activates the display the same way.
            if result {
                selectionDisplay?.isActivated = true
            }
            return result
        }

        @discardableResult
        override func resignFirstResponder() -> Bool {
            commitPendingSelection()
            let result = super.resignFirstResponder()
            if result {
                selectionDisplay?.isActivated = false
            }
            return result
        }

        // MARK: - Frame output

        /// Apply one frame's output. Only changes the editor made itself are
        /// reported to UIKit; edits UIKit made were applied synchronously.
        func apply(_ output: IOSResponse) {
            if pendingSelection != nil, !(page?.touches.touching ?? false) {
                textInteractionActive = false
                commitPendingSelection()
            }
            if output.has_virtual_keyboard_shown {
                if output.virtual_keyboard_shown, !isFirstResponder {
                    becomeFirstResponder()
                } else if !output.virtual_keyboard_shown, isFirstResponder {
                    resignFirstResponder()
                }
            }

            if output.text_updated {
                marked = nil
                inputDelegate?.textWillChange(self)
                inputDelegate?.textDidChange(self)
            }
            if output.text_updated || output.selection_updated {
                inputDelegate?.selectionWillChange(self)
                inputDelegate?.selectionDidChange(self)
            }

            // Scroll and layout move geometry without UIKit knowing. While
            // UIKit drags a handle or the loupe it places those views itself,
            // and a layout resets the handle it is placing, so then only when
            // the selection's ends moved under them.
            selectionMoved = selectionGeometryMoved()
            if page?.textDragMoving != true || selectionMoved, let display = selectionDisplay {
                display.setNeedsSelectionUpdate()
                display.layoutManagedSubviews()
            }

            if let wsHandle {
                update_virtual_keyboard(wsHandle, isFirstResponder && GCKeyboard.coalesced == nil)
            }
        }

        /// The selection's ends moved on screen this frame: the page scrolled
        /// or the layout shifted under them.
        private(set) var selectionMoved = false
        private var lastSelectionGeometry: [CGRect] = []

        private func selectionGeometryMoved() -> Bool {
            guard let wsHandle else { return false }
            let range = get_selected(wsHandle)
            let now = range.none ? [] : [range.start, range.end].map {
                localRect(cursor_rect_at_position(wsHandle, $0)) ?? .null
            }
            defer { lastSelectionGeometry = now }
            return now != lastSelectionGeometry
        }

        /// A different document is behind this view now.
        func documentReplaced() {
            pendingSelection = nil
            textInteractionActive = false
            focusForHardwareKeyboard()
            marked = nil
            inputDelegate?.textWillChange(self)
            inputDelegate?.textDidChange(self)
            inputDelegate?.selectionWillChange(self)
            inputDelegate?.selectionDidChange(self)
        }

        private var selectionDisplay: UITextSelectionDisplayInteraction? {
            interactions.lazy.compactMap { $0 as? UITextSelectionDisplayInteraction }.first
        }

        /// After a UIKit edit: lay out now so the next geometry query sees it.
        private func layoutNow() {
            mtkView.layoutFrame()
        }

        /// A scroll dismisses the edit menu, as in Notes.
        func dismissEditMenus() {
            let menus = interactions.compactMap { $0 as? UIEditMenuInteraction }
            menus.forEach { $0.dismissMenu() }
        }

        // MARK: - Hardware keys

        /// Each key has one owner. UIKit's text system takes text keys:
        /// characters, arrows, deletes, Return, Tab, and the standard edit
        /// chords. The editor takes its own shortcuts.
        private var editorKeys = Set<UIKeyboardHIDUsage>()

        override func pressesBegan(_ presses: Set<UIPress>, with event: UIPressesEvent?) {
            var system = Set<UIPress>()
            for press in presses {
                guard let key = press.key else {
                    system.insert(press)
                    continue
                }
                if isEditorKey(key) {
                    editorKeys.insert(key.keyCode)
                    sendKey(key, pressed: true)
                } else {
                    syncModifier(key, pressed: true)
                    system.insert(press)
                }
            }
            if !system.isEmpty {
                super.pressesBegan(system, with: event)
            }
        }

        override func pressesEnded(_ presses: Set<UIPress>, with event: UIPressesEvent?) {
            super.pressesEnded(releaseEditorKeys(presses), with: event)
        }

        override func pressesCancelled(_ presses: Set<UIPress>, with event: UIPressesEvent?) {
            super.pressesCancelled(releaseEditorKeys(presses), with: event)
        }

        /// Sends the editor its key-ups; returns the presses UIKit owns.
        private func releaseEditorKeys(_ presses: Set<UIPress>) -> Set<UIPress> {
            presses.filter { press in
                guard let key = press.key else { return true }
                if editorKeys.remove(key.keyCode) != nil {
                    sendKey(key, pressed: false)
                    return false
                }
                syncModifier(key, pressed: false)
                return true
            }
        }

        /// A modifier key's own press or release, so a chord's key-up does
        /// not leave its modifier held in the editor.
        private func syncModifier(_ key: UIKey, pressed: Bool) {
            guard let wsHandle else { return }
            let own: UIKeyModifierFlags
            switch key.keyCode {
            case .keyboardLeftGUI, .keyboardRightGUI: own = .command
            case .keyboardLeftShift, .keyboardRightShift: own = .shift
            case .keyboardLeftControl, .keyboardRightControl: own = .control
            case .keyboardLeftAlt, .keyboardRightAlt: own = .alternate
            default: return
            }
            var mods = key.modifierFlags
            if pressed {
                mods.insert(own)
            } else {
                mods.remove(own)
            }
            ios_key_event(
                wsHandle, key.keyCode.rawValue, mods.contains(.shift), mods.contains(.control),
                mods.contains(.alternate), mods.contains(.command), pressed
            )
            mtkView.requestFrame()
        }

        private func isEditorKey(_ key: UIKey) -> Bool {
            let mods = key.modifierFlags
            switch key.keyCode {
            case .keyboardEscape:
                return true
            case .keyboardTab:
                return mods.contains(.shift)
            case .keyboardLeftArrow, .keyboardRightArrow, .keyboardUpArrow, .keyboardDownArrow,
                 .keyboardDeleteOrBackspace, .keyboardDeleteForward, .keyboardReturnOrEnter,
                 .keyboardLeftShift, .keyboardRightShift, .keyboardLeftControl, .keyboardRightControl,
                 .keyboardLeftAlt, .keyboardRightAlt, .keyboardLeftGUI, .keyboardRightGUI,
                 .keyboardCapsLock:
                return false
            default:
                guard mods.contains(.command) || mods.contains(.control) else { return false }
                let standard: Set<UIKeyboardHIDUsage> = [.keyboardA, .keyboardC, .keyboardV, .keyboardX, .keyboardZ]
                return !(mods.contains(.command) && !mods.contains(.control) && standard.contains(key.keyCode))
            }
        }

        private func sendKey(_ key: UIKey, pressed: Bool) {
            guard let wsHandle else { return }
            let mods = key.modifierFlags
            ios_key_event(
                wsHandle, key.keyCode.rawValue, mods.contains(.shift), mods.contains(.control),
                mods.contains(.alternate), mods.contains(.command), pressed
            )
            mtkView.requestFrame()
        }

        /// The system claims these chords before `pressesBegan`.
        override var keyCommands: [UIKeyCommand]? {
            iOSMTK.workspaceBracketKeyCommands()
        }

        @objc func forwardBracketCommand(_ command: UIKeyCommand) {
            mtkView.forwardBracketCommand(command)
        }

        // MARK: - Undo

        override var undoManager: UndoManager? { history }

        /// Command-Z and shift-command-Z arrive up the responder chain; taken
        /// here so an ancestor's own history does not answer them.
        @objc func undo(_: Any?) {
            applyHistory(redo: false)
        }

        @objc func redo(_: Any?) {
            applyHistory(redo: true)
        }

        /// A change UIKit did not make, so it hears of it around the edit.
        func applyHistory(redo: Bool) {
            commitPendingSelection()
            guard let wsHandle else { return }
            marked = nil
            inputDelegate?.textWillChange(self)
            inputDelegate?.selectionWillChange(self)
            undo_redo(wsHandle, redo)
            layoutNow()
            inputDelegate?.selectionDidChange(self)
            inputDelegate?.textDidChange(self)
        }

        // MARK: - UIKeyInput

        var hasText: Bool {
            guard let wsHandle else { return false }
            return end_of_document(wsHandle).pos > 0
        }

        func insertText(_ text: String) {
            commitPendingSelection()
            guard let wsHandle else { return }
            let before = selectedTextRange as? TextRange
            if let marked {
                self.marked = nil
                text.withCString { replace_text(wsHandle, cRange(marked.lo, marked.hi), $0) }
            } else {
                text.withCString { insert_text(wsHandle, $0) }
            }
            rememberStaleCaret(before)
            layoutNow()
        }

        /// After a synchronous edit UIKit asks once more for the caret at a
        /// position it held, which the edit moved: answered as the new caret
        /// until UIKit asks for the current one.
        private var staleCarets: [Int] = []

        private func rememberStaleCaret(_ before: TextRange?) {
            guard let before, let now = (selectedTextRange as? TextRange)?.lo else { return }
            staleCarets = [before.lo, before.hi].filter { $0 != now }
        }

        func deleteBackward() {
            commitPendingSelection()
            guard let wsHandle else { return }
            marked = nil
            let before = selectedTextRange as? TextRange
            backspace(wsHandle)
            rememberStaleCaret(before)
            layoutNow()
            // Deleting a range leaves UIKit's selection display at its old end.
            inputDelegate?.selectionDidChange(nil)
        }

        // MARK: - UITextInput: text

        func text(in range: UITextRange) -> String? {
            guard let wsHandle, let range = range as? TextRange else { return nil }
            guard let result = text_in_range(wsHandle, cRange(range.lo, range.hi)) else { return nil }
            let text = String(cString: result)
            free_text(result)
            return text
        }

        func replace(_ range: UITextRange, withText text: String) {
            commitPendingSelection()
            guard let wsHandle, let range = range as? TextRange else { return }
            marked = nil
            text.withCString { replace_text(wsHandle, cRange(range.lo, range.hi), $0) }
            layoutNow()
        }

        /// UIKit's writes during a tap, loupe, or handle drag, held until it
        /// ends: the note neither reveals nor reflows under a moving caret.
        private var pendingSelection: TextRange?

        var selectedTextRange: UITextRange? {
            get {
                if let floating { return floating.target }
                if let pendingSelection { return pendingSelection }
                guard let wsHandle else { return nil }
                let range = get_selected(wsHandle)
                if range.none { return nil }
                return TextRange(Int(range.start.pos), Int(range.end.pos))
            }
            set {
                guard let wsHandle, let range = newValue as? TextRange else { return }
                if let floating {
                    floating.target = range
                    placeFloatingTarget()
                    return
                }
                let dragging = page?.textDragMoving ?? false
                if dragging {
                    // Held until the lift, whether UIKit's write or the
                    // crawl's: the note neither reveals nor reflows under
                    // the moving end.
                    pendingSelection = range
                    mtkView.requestFrame()
                    return
                }
                // A write outside a drag stands on its own; a held one is
                // superseded, as the lift's write supersedes the crawl's.
                pendingSelection = nil
                if textInteractionActive {
                    // A tap's write applies now, and the keyboard hears of it
                    // now: its spelling bubble and UIKit's double-tap menu are
                    // decided while the tap is still in hand.
                    inputDelegate?.selectionWillChange(self)
                    set_selected(wsHandle, cRange(range.lo, range.hi), false)
                    inputDelegate?.selectionDidChange(self)
                    mtkView.requestFrame()
                    return
                }
                // A handle drag writes at lift, with the moved end under the
                // finger: no reveal, which would scroll to the far end.
                let reveal = !(page?.textDragActive ?? false)
                set_selected(wsHandle, cRange(range.lo, range.hi), reveal)
                mtkView.requestFrame()
            }
        }

        /// The editor takes the range UIKit last wrote. The caret is under
        /// the finger that placed it, so no reveal scroll. The keyboard is
        /// told: it computed its context (capitalization, suggestions) at
        /// the old caret before the tap and only recomputes on this pair.
        private func commitPendingSelection() {
            guard let range = pendingSelection, let wsHandle else { return }
            pendingSelection = nil
            // The page is gone: the write was for a note no longer here.
            guard page != nil else { return }
            inputDelegate?.selectionWillChange(self)
            set_selected(wsHandle, cRange(range.lo, range.hi), false)
            inputDelegate?.selectionDidChange(self)
            mtkView.requestFrame()
        }

        // MARK: - Spacebar cursor

        /// The blue cursor follows the finger; a grey caret shows where it
        /// lands. The selection commits on lift, so the note does not reveal
        /// and reflow under the moving cursor.
        private final class FloatingCursor {
            var target: TextRange?
            let finger: UIView
            let landing: UIView
            /// The finger's travel is windowed like a mouse's: a push past
            /// an edge slides the window, so coming back moves the cursor at
            /// once. Points from UIKit map into the view by this origin.
            var window: CGRect

            init(finger: UIView, landing: UIView, window: CGRect) {
                self.finger = finger
                self.landing = landing
                self.window = window
            }

            func slide(to point: CGPoint) -> CGPoint {
                if point.x < window.minX { window.origin.x = point.x }
                if point.x > window.maxX { window.origin.x = point.x - window.width }
                if point.y < window.minY { window.origin.y = point.y }
                if point.y > window.maxY { window.origin.y = point.y - window.height }
                return CGPoint(x: point.x - window.minX, y: point.y - window.minY)
            }
        }

        private var floating: FloatingCursor?

        func beginFloatingCursor(at point: CGPoint) {
            guard floating == nil else { return }
            let target = selectedTextRange as? TextRange
            let finger = cursorView(tintColor)
            let landing = cursorView(.systemGray)
            for view in [landing, finger] {
                view.isUserInteractionEnabled = false
                addSubview(view)
            }
            let cursor = FloatingCursor(finger: finger, landing: landing, window: bounds)
            cursor.target = target
            floating = cursor
            selectionDisplay?.cursorView.isHidden = true
            placeFloatingTarget()
            updateFloatingCursor(at: point)
        }

        func updateFloatingCursor(at point: CGPoint) {
            guard let floating else { return }
            guard point.x.isFinite, point.y.isFinite, floating.landing.bounds.size.height.isFinite else {
                return
            }
            let local = floating.slide(to: point)
            let size = floating.landing.bounds.size
            let x = min(max(local.x, 0), bounds.width)
            let y = min(max(local.y, size.height / 2), bounds.height - size.height / 2)
            floating.finger.bounds.size = size
            floating.finger.center = CGPoint(x: x, y: y)
        }

        func endFloatingCursor() {
            guard let cursor = floating else { return }
            floating = nil
            cursor.finger.removeFromSuperview()
            cursor.landing.removeFromSuperview()
            selectionDisplay?.cursorView.isHidden = false
            // The keyboard recomputes its suggestions on this pair, as after
            // a tap.
            if let target = cursor.target, let wsHandle {
                inputDelegate?.selectionWillChange(self)
                set_selected(wsHandle, cRange(target.lo, target.hi), false)
                inputDelegate?.selectionDidChange(self)
                mtkView.requestFrame()
            }
        }

        private func cursorView(_ color: UIColor) -> UIView {
            let view = UIStandardTextCursorView()
            view.tintColor = color
            view.isBlinking = false
            return view
        }

        private func placeFloatingTarget() {
            guard let floating, let target = floating.target else { return }
            let rect = caretRect(for: target.end)
            guard !rect.isNull else { return }
            floating.landing.frame = rect
            if floating.finger.bounds.size == .zero {
                floating.finger.bounds.size = rect.size
            }
        }

        // MARK: - Edit menu actions

        override func canPerformAction(_ action: Selector, withSender sender: Any?) -> Bool {
            let selection = selectedTextRange
            switch action {
            case #selector(copy(_:)):
                return selection?.isEmpty == false
            case #selector(cut(_:)):
                return selection?.isEmpty == false
            case #selector(paste(_:)):
                return UIPasteboard.general.hasStrings || UIPasteboard.general.hasImages
                    || UIPasteboard.general.hasURLs
            case #selector(select(_:)):
                return selection?.isEmpty == true && hasText
            case #selector(selectAll(_:)):
                return hasText
            default:
                return super.canPerformAction(action, withSender: sender)
            }
        }

        override func copy(_: Any?) {
            guard let range = selectedTextRange, let text = text(in: range), !text.isEmpty else { return }
            UIPasteboard.general.string = text
        }

        override func cut(_ sender: Any?) {
            commitPendingSelection()
            guard let range = selectedTextRange, !range.isEmpty else { return }
            copy(sender)
            inputDelegate?.selectionWillChange(self)
            replace(range, withText: "")
            inputDelegate?.selectionDidChange(self)
        }

        /// The editor's paste: a URL over a selection becomes a link. Text
        /// applies now, like any UIKit edit; an image arrives next frame.
        override func paste(_: Any?) {
            commitPendingSelection()
            guard let wsHandle else { return }
            if let image = UIPasteboard.general.image {
                mtkView.importContent(.image(image), isPaste: true)
                mtkView.requestFrame()
            } else if let text = UIPasteboard.general.string ?? UIPasteboard.general.url?.absoluteString {
                inputDelegate?.selectionWillChange(self)
                text.withCString { paste_text(wsHandle, $0) }
                layoutNow()
                inputDelegate?.selectionDidChange(self)
            }
        }

        override func select(_: Any?) {
            guard let caret = selectedTextRange?.start,
                  let word = tokenizer.rangeEnclosingPosition(caret, with: .word, inDirection: .storage(.backward))
                  ?? tokenizer.rangeEnclosingPosition(caret, with: .word, inDirection: .storage(.forward))
            else { return }
            inputDelegate?.selectionWillChange(self)
            selectedTextRange = word
            inputDelegate?.selectionDidChange(self)
        }

        override func selectAll(_: Any?) {
            inputDelegate?.selectionWillChange(self)
            selectedTextRange = TextRange(0, endOffset)
            inputDelegate?.selectionDidChange(self)
        }

        // MARK: - UITextInput: marked text

        var markedTextRange: UITextRange? {
            marked.map { TextRange($0.lo, $0.hi) }
        }

        func setMarkedText(_ markedText: String?, selectedRange: NSRange) {
            commitPendingSelection()
            guard let wsHandle else { return }
            let text = markedText ?? ""
            let target = marked ?? currentRange()
            text.withCString { replace_text(wsHandle, cRange(target.lo, target.hi), $0) }
            let length = text.utf16.count
            marked = length > 0 ? (target.lo, target.lo + length) : nil

            let lo = target.lo + selectedRange.location
            let hi = lo + selectedRange.length
            inputDelegate?.selectionWillChange(self)
            set_selected(wsHandle, cRange(lo, hi), true)
            inputDelegate?.selectionDidChange(self)
            layoutNow()
        }

        func unmarkText() {
            guard marked != nil else { return }
            inputDelegate?.selectionWillChange(self)
            marked = nil
            inputDelegate?.selectionDidChange(self)
        }

        private func currentRange() -> (lo: Int, hi: Int) {
            guard let range = selectedTextRange as? TextRange else { return (0, 0) }
            return (range.lo, range.hi)
        }

        // MARK: - UITextInput: positions

        var beginningOfDocument: UITextPosition { TextPosition(0) }

        var endOfDocument: UITextPosition {
            guard let wsHandle else { return TextPosition(0) }
            return TextPosition(Int(end_of_document(wsHandle).pos))
        }

        private var endOffset: Int {
            (endOfDocument as? TextPosition)?.offset ?? 0
        }

        func textRange(from fromPosition: UITextPosition, to toPosition: UITextPosition) -> UITextRange? {
            guard let from = fromPosition as? TextPosition, let to = toPosition as? TextPosition else {
                return nil
            }
            return TextRange(min(from.offset, to.offset), max(from.offset, to.offset))
        }

        func position(from position: UITextPosition, offset: Int) -> UITextPosition? {
            guard let position = position as? TextPosition else { return nil }
            let result = position.offset + offset
            guard result >= 0, result <= endOffset else { return nil }
            return TextPosition(result)
        }

        func position(
            from position: UITextPosition, in direction: UITextLayoutDirection, offset: Int
        ) -> UITextPosition? {
            guard let wsHandle, let position = position as? TextPosition else { return nil }
            let result = position_offset_in_direction(
                wsHandle, cPosition(position.offset),
                CTextLayoutDirection(rawValue: UInt32(direction.rawValue)), Int32(offset)
            )
            return result.none ? nil : TextPosition(Int(result.pos))
        }

        func compare(_ position: UITextPosition, to other: UITextPosition) -> ComparisonResult {
            let a = (position as? TextPosition)?.offset ?? 0
            let b = (other as? TextPosition)?.offset ?? 0
            return a < b ? .orderedAscending : a > b ? .orderedDescending : .orderedSame
        }

        func offset(from: UITextPosition, to toPosition: UITextPosition) -> Int {
            let a = (from as? TextPosition)?.offset ?? 0
            let b = (toPosition as? TextPosition)?.offset ?? 0
            return b - a
        }

        func position(within range: UITextRange, farthestIn direction: UITextLayoutDirection) -> UITextPosition? {
            return switch direction {
            case .left, .up: range.start
            case .right, .down: range.end
            @unknown default: nil
            }
        }

        func characterRange(
            byExtending position: UITextPosition, in direction: UITextLayoutDirection
        ) -> UITextRange? {
            guard let next = self.position(from: position, in: direction, offset: 1) else { return nil }
            return textRange(from: position, to: next)
        }

        func baseWritingDirection(
            for _: UITextPosition, in _: UITextStorageDirection
        ) -> NSWritingDirection {
            .leftToRight
        }

        func setBaseWritingDirection(_: NSWritingDirection, for _: UITextRange) {}

        // MARK: - UITextInput: geometry

        func firstRect(for range: UITextRange) -> CGRect {
            guard let wsHandle, let range = range as? TextRange else { return lastCaretRect }
            return localRect(first_rect(wsHandle, cRange(range.lo, range.hi))) ?? caretRect(for: range.start)
        }

        /// Never null: UIKit animates its caret from the last rect, and a
        /// null one puts NaN in that animation. A position with no geometry
        /// (off screen, mid-layout) answers with the last caret rect.
        func caretRect(for position: UITextPosition) -> CGRect {
            if let wsHandle, let offset = (position as? TextPosition)?.offset, !staleCarets.isEmpty {
                let now = (selectedTextRange as? TextRange)?.lo
                if staleCarets.contains(offset), let now, var rect = localRect(cursor_rect_at_position(wsHandle, cPosition(now))) {
                    rect.size.width = 2
                    return rect
                }
                // UIKit has moved on to the caret the edit left.
                if offset == now {
                    staleCarets = []
                }
            }
            guard let wsHandle, let position = position as? TextPosition else { return lastCaretRect }
            guard var rect = localRect(cursor_rect_at_position(wsHandle, cPosition(position.offset))) else {
                return lastCaretRect
            }
            rect.size.width = 2
            lastCaretRect = rect
            return rect
        }

        private var lastCaretRect = CGRect(x: 0, y: 0, width: 2, height: 22)

        func selectionRects(for range: UITextRange) -> [UITextSelectionRect] {
            guard let wsHandle, let range = range as? TextRange, !range.isEmpty else { return [] }
            let rects = selection_rects(wsHandle, cRange(range.lo, range.hi))
            defer { free_selection_rects(rects) }
            let count = Int(rects.size)
            let out = (0..<count).compactMap { i in
                localRect(rects.rects[i]).map {
                    TextSelectionRect($0, containsStart: i == 0, containsEnd: i == count - 1)
                }
            }
            return out
        }

        func closestPosition(to point: CGPoint) -> UITextPosition? {
            guard let wsHandle else { return nil }
            let point = floating.map { CGPoint(x: point.x - $0.window.minX, y: point.y - $0.window.minY) } ?? point
            let p = convert(point, to: mtkView)
            let result = position_at_point(wsHandle, CPoint(x: p.x, y: p.y))
            return result.none ? nil : TextPosition(Int(result.pos))
        }

        func closestPosition(to point: CGPoint, within range: UITextRange) -> UITextPosition? {
            guard let range = range as? TextRange,
                  let position = closestPosition(to: point) as? TextPosition
            else { return nil }
            return TextPosition(min(max(position.offset, range.lo), range.hi))
        }

        func characterRange(at point: CGPoint) -> UITextRange? {
            guard let position = closestPosition(to: point) as? TextPosition else { return nil }
            let end = endOffset
            if position.offset < end {
                return TextRange(position.offset, position.offset + 1)
            } else if end > 0 {
                return TextRange(end - 1, end)
            }
            return nil
        }

        /// Editor rect (Metal view points) in this view's space; nil when the
        /// editor has no geometry for it (e.g. off screen).
        private func localRect(_ r: CRect) -> CGRect? {
            if r.min_x == 0, r.min_y == 0, r.max_x == 0, r.max_y == 0 { return nil }
            guard [r.min_x, r.min_y, r.max_x, r.max_y].allSatisfy(\.isFinite) else {
                return nil
            }
            let rect = CGRect(x: r.min_x, y: r.min_y, width: r.max_x - r.min_x, height: r.max_y - r.min_y)
            return convert(rect, from: mtkView)
        }

        // MARK: - Gesture relationships

        override func gestureRecognizerShouldBegin(_ gestureRecognizer: UIGestureRecognizer) -> Bool {
            if isInteractiveContentPop(gestureRecognizer) {
                return false
            }
            return super.gestureRecognizerShouldBegin(gestureRecognizer)
        }
    }

    extension TextInputView: UIGestureRecognizerDelegate {
        /// The touch watch runs beside everything.
        func gestureRecognizer(
            _ gestureRecognizer: UIGestureRecognizer,
            shouldRecognizeSimultaneouslyWith otherGestureRecognizer: UIGestureRecognizer
        ) -> Bool {
            gestureRecognizer === page?.touches || otherGestureRecognizer === page?.touches
        }
    }

    extension TextInputView: UITextInteractionDelegate {
        func interactionShouldBegin(_ interaction: UITextInteraction, at point: CGPoint) -> Bool {
            return true
        }

        func interactionWillBegin(_: UITextInteraction) {
            textInteractionActive = true
        }

        func interactionDidEnd(_: UITextInteraction) {
            textInteractionActive = false
            commitPendingSelection()
        }
    }

    // MARK: - Menus

    extension TextInputView {
        /// The text interaction's own menu: link actions join the standard ones.
        func editMenu(for _: UITextRange, suggestedActions: [UIMenuElement]) -> UIMenu? {
            UIMenu(children: editorMenuItems() + suggestedActions.withoutUndo)
        }

        /// Open and Copy for a selected link.
        private func editorMenuItems() -> [UIMenuElement] {
            guard let wsHandle else { return [] }
            var items: [UIMenuElement] = []
            if let target = selection_open_target(wsHandle) {
                let url = String(cString: target)
                free_text(target)
                items.append(UIAction(title: "Open Link", image: UIImage(systemName: "arrow.up.forward.app")) { [weak self] _ in
                    open_selection_links(wsHandle)
                    self?.mtkView.requestFrame()
                })
                items.append(UIAction(title: "Copy Link", image: UIImage(systemName: "link")) { _ in
                    UIPasteboard.general.string = url
                })
            }
            return items
        }
    }

    extension [UIMenuElement] {
        /// Undo lives on shake and command-Z, not the selection menu.
        var withoutUndo: [UIMenuElement] {
            compactMap { element in
                if let command = element as? UICommand,
                   command.action == NSSelectorFromString("undo:")
                   || command.action == NSSelectorFromString("redo:")
                {
                    return nil
                }
                if let menu = element as? UIMenu {
                    return menu.replacingChildren(menu.children.withoutUndo)
                }
                return element
            }
        }
    }

    // MARK: - Drop

    extension TextInputView: UIDropInteractionDelegate {
        func dropInteraction(_: UIDropInteraction, canHandle session: UIDropSession) -> Bool {
            session.items.count == 1 && session.hasItemsConforming(toTypeIdentifiers: [
                UTType.image.identifier, UTType.fileURL.identifier, UTType.text.identifier,
            ])
        }

        func dropInteraction(_: UIDropInteraction, sessionDidUpdate _: UIDropSession) -> UIDropProposal {
            UIDropProposal(operation: .copy)
        }

        /// Through the editor's import, like paste; the frame that applies it
        /// reports the change to UIKit.
        func dropInteraction(_: UIDropInteraction, performDrop session: UIDropSession) {
            let mtkView = mtkView
            if session.hasItemsConforming(toTypeIdentifiers: [UTType.image.identifier]) {
                _ = session.loadObjects(ofClass: UIImage.self) { images in
                    for case let image as UIImage in images {
                        mtkView.importContent(.image(image), isPaste: true)
                    }
                    mtkView.requestFrame()
                }
            } else if session.hasItemsConforming(toTypeIdentifiers: [UTType.fileURL.identifier]) {
                _ = session.loadObjects(ofClass: URL.self) { urls in
                    for url in urls {
                        mtkView.importContent(.url(url), isPaste: true)
                    }
                    mtkView.requestFrame()
                }
            } else {
                _ = session.loadObjects(ofClass: String.self) { strings in
                    for string in strings {
                        mtkView.importContent(.text(string), isPaste: true)
                    }
                    mtkView.requestFrame()
                }
            }
        }
    }

    // MARK: - Undo

    /// The editor's history, for shake-to-undo and command-Z.
    final class EditorUndoManager: UndoManager {
        weak var view: TextInputView?

        override var canUndo: Bool { view?.wsHandle.map { can_undo($0) } ?? false }
        override var canRedo: Bool { view?.wsHandle.map { can_redo($0) } ?? false }

        override func undo() {
            view?.applyHistory(redo: false)
        }

        override func redo() {
            view?.applyHistory(redo: true)
        }
    }

    // MARK: - Recognizers

    /// Recognizes at touch-down when that touch stopped a coasting page, so
    /// the stop places no caret. Fails at once otherwise, so nothing waits.
    final class CoastStopRecognizer: UIGestureRecognizer {
        var stopCoast: () -> Bool = { false }

        override init(target: Any?, action: Selector?) {
            super.init(target: target, action: action)
            delaysTouchesBegan = false
            delaysTouchesEnded = false
            cancelsTouchesInView = false
        }

        convenience init() {
            self.init(target: nil, action: nil)
        }

        override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent) {
            guard state == .possible else { return }
            state = stopCoast() ? .recognized : .failed
        }
    }

    extension UIGestureRecognizer {
        /// UIKit's selection-handle drag. Named in Runestone the same way.
        var isRangeAdjustment: Bool {
            (name ?? "").contains("RangeAdjustment")
                || NSStringFromClass(type(of: self)).contains("RangeAdjustment")
        }

        /// UIKit's caret drag under the loupe.
        var isCaretDrag: Bool {
            (name ?? "").contains("InteractiveRefinement")
                || NSStringFromClass(type(of: self)).contains("Loupe")
        }

        /// UIKit's tap-then-drag selection.
        var isSelectionDrag: Bool {
            (name ?? "").contains("TapAndAHalf")
        }
    }

    // MARK: - Tokenizer

    /// The editor's text units, read in place, for every granularity: Apple's
    /// word, sentence, and paragraph rules, and lines as the editor's wrap
    /// rows, each ending before its newline.
    final class EditorTokenizer: NSObject, UITextInputTokenizer {
        private weak var view: TextInputView?

        init(textInput: TextInputView) {
            view = textInput
        }

        func rangeEnclosingPosition(
            _ position: UITextPosition, with granularity: UITextGranularity,
            inDirection direction: UITextDirection
        ) -> UITextRange? {
            guard let wsHandle = view?.wsHandle, let p = position as? TextPosition else { return nil }
            let r = unit_enclosing(wsHandle, cPosition(p.offset), granularity.c, direction.isBackward)
            return r.none ? nil : TextRange(Int(r.start.pos), Int(r.end.pos))
        }

        func position(
            from position: UITextPosition, toBoundary granularity: UITextGranularity,
            inDirection direction: UITextDirection
        ) -> UITextPosition? {
            guard let wsHandle = view?.wsHandle, let p = position as? TextPosition else { return nil }
            let r = unit_boundary_from(wsHandle, cPosition(p.offset), granularity.c, direction.isBackward)
            return r.none ? nil : TextPosition(Int(r.pos))
        }

        func isPosition(
            _ position: UITextPosition, atBoundary granularity: UITextGranularity,
            inDirection direction: UITextDirection
        ) -> Bool {
            guard let wsHandle = view?.wsHandle, let p = position as? TextPosition else { return false }
            return unit_at_boundary(wsHandle, cPosition(p.offset), granularity.c, direction.isBackward)
        }

        func isPosition(
            _ position: UITextPosition, withinTextUnit granularity: UITextGranularity,
            inDirection direction: UITextDirection
        ) -> Bool {
            guard let wsHandle = view?.wsHandle, let p = position as? TextPosition else { return false }
            return unit_within(wsHandle, cPosition(p.offset), granularity.c, direction.isBackward)
        }
    }

    extension UITextGranularity {
        var c: CTextGranularity { CTextGranularity(rawValue: UInt32(rawValue)) }
    }

    extension UITextDirection {
        /// Storage backward, or layout left or up.
        var isBackward: Bool {
            self == .storage(.backward) || self == .layout(.left) || self == .layout(.up)
        }
    }

    // MARK: - Positions

    final class TextPosition: UITextPosition {
        let offset: Int

        init(_ offset: Int) {
            self.offset = offset
        }

        override func isEqual(_ object: Any?) -> Bool {
            (object as? TextPosition)?.offset == offset
        }

        override var hash: Int { offset }
    }

    final class TextRange: UITextRange {
        let lo: Int
        let hi: Int

        init(_ lo: Int, _ hi: Int) {
            self.lo = lo
            self.hi = hi
        }

        override var start: UITextPosition { TextPosition(lo) }
        override var end: UITextPosition { TextPosition(hi) }
        override var isEmpty: Bool { lo >= hi }

        override func isEqual(_ object: Any?) -> Bool {
            guard let other = object as? TextRange else { return false }
            return other.lo == lo && other.hi == hi
        }

        override var hash: Int { lo &* 31 &+ hi }
    }

    final class TextSelectionRect: UITextSelectionRect {
        private let _rect: CGRect
        private let _containsStart: Bool
        private let _containsEnd: Bool

        init(_ rect: CGRect, containsStart: Bool, containsEnd: Bool) {
            _rect = rect
            _containsStart = containsStart
            _containsEnd = containsEnd
        }

        override var rect: CGRect { _rect }
        override var writingDirection: NSWritingDirection { .leftToRight }
        override var containsStart: Bool { _containsStart }
        override var containsEnd: Bool { _containsEnd }
        override var isVertical: Bool { false }
    }

    private func cPosition(_ offset: Int) -> CTextPosition {
        CTextPosition(none: false, pos: UInt(max(offset, 0)))
    }

    private func cRange(_ lo: Int, _ hi: Int) -> CTextRange {
        CTextRange(none: false, start: cPosition(lo), end: cPosition(hi))
    }
#endif
