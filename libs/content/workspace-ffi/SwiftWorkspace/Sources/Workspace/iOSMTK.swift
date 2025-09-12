#if os(iOS)
import UIKit
import MetalKit
import Bridge
import SwiftUI
import MobileCoreServices
import UniformTypeIdentifiers

import GameController

public class iOSMTKTextInputWrapper: UIView, UITextInput, UIDropInteractionDelegate {
    public static let TOOL_BAR_HEIGHT: CGFloat = 42
    public static let FLOATING_CURSOR_OFFSET_HEIGHT: CGFloat = 0.6

    let mtkView: iOSMTK

    var textUndoManager = iOSUndoManager()
    let textInteraction = UITextInteraction(for: .editable)

    var wsHandle: UnsafeMutableRawPointer? { get { mtkView.wsHandle } }
    var workspaceState: WorkspaceState? { get { mtkView.workspaceState } }
    
    public override var undoManager: UndoManager? {
        return textUndoManager
    }

    var lastFloatingCursorRect: CGRect? = nil
    var floatingCursor: UIView = UIView()
    var floatingCursorWidth = 1.0
    var floatingCursorNewStartX = 0.0
    var floatingCursorNewEndX = 0.0
    var floatingCursorNewStartY = 0.0
    var floatingCursorNewEndY = 0.0
    var autoScroll: Timer? = nil
        
    var isLongPressCursorDrag = false
    
    init(mtkView: iOSMTK) {
        self.mtkView = mtkView

        super.init(frame: .infinite)

        mtkView.onSelectionChanged = { [weak self] in
            self?.inputDelegate?.selectionDidChange(self)
        }
        mtkView.onTextChanged = { [weak self] in
            self?.inputDelegate?.textDidChange(self)
        }

        self.clipsToBounds = true
        self.isUserInteractionEnabled = true

        // ipad trackpad support
        let pan = UIPanGestureRecognizer(target: self, action: #selector(self.handleTrackpadScroll(_:)))
        pan.allowedScrollTypesMask = .all
        pan.maximumNumberOfTouches = 0
        self.addGestureRecognizer(pan)

        // selection support
        textInteraction.textInput = self
        self.addInteraction(textInteraction)

        for gestureRecognizer in textInteraction.gesturesForFailureRequirements {
            let gestureName = gestureRecognizer.name?.lowercased()

            if gestureName?.contains("tap") ?? false {
                gestureRecognizer.cancelsTouchesInView = false
            }
        }
        
        for gesture in gestureRecognizers ?? [] {
            let gestureName = gesture.name?.lowercased()
            
            if gestureName?.contains("interactiverefinement") ?? false {
                gesture.addTarget(self, action: #selector(longPressGestureStateChanged(_:)))
            }
        }
        
        // drop support
        let dropInteraction = UIDropInteraction(delegate: self)
        self.addInteraction(dropInteraction)

        // undo redo support
        self.textUndoManager.textWrapper = self
        self.textUndoManager.wsHandle = self.wsHandle
        self.textUndoManager.mtkView = mtkView
        self.textUndoManager.inputDelegate = inputDelegate
        
        // floating cursor support
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
    
    @objc func handleTrackpadScroll(_ sender: UIPanGestureRecognizer? = nil) {
        mtkView.handleTrackpadScroll(sender)
    }
        
    @objc private func longPressGestureStateChanged(_ recognizer: UIGestureRecognizer) {
        switch recognizer.state {
        case .began:
            isLongPressCursorDrag = true
        case .cancelled, .ended:
            isLongPressCursorDrag = false
            
            inputDelegate?.selectionWillChange(self)
            mtkView.drawImmediately()
            inputDelegate?.selectionDidChange(self)
        default:
            break
        }
    }
            
    required init(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    public override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent?) {
        mtkView.touchesBegan(touches, with: event)
    }

    public override func touchesMoved(_ touches: Set<UITouch>, with event: UIEvent?) {
        mtkView.touchesMoved(touches, with: event)
    }

    public override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent?) {
        mtkView.touchesEnded(touches, with: event)
    }

    public override func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent?) {
        mtkView.touchesCancelled(touches, with: event)
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
        if(point.x < floatingCursorNewStartX) {
            floatingCursorNewEndX -= (floatingCursorNewStartX - point.x)
            floatingCursorNewStartX = point.x
        }

        if(point.x > floatingCursorNewEndX) {
            floatingCursorNewStartX += (point.x - floatingCursorNewEndX)
            floatingCursorNewEndX = point.x
        }

        if(point.y < floatingCursorNewStartY) {
            floatingCursorNewEndY -= (floatingCursorNewStartY - point.y)
            floatingCursorNewStartY = point.y
        }

        if(point.y > floatingCursorNewEndY) {
            floatingCursorNewStartY += (point.y - floatingCursorNewEndY)
            floatingCursorNewEndY = point.y
        }

        setFloatingCursorLoc(point: point, animate: true)
    }

    public func endFloatingCursor() {
        if let cursorRect = lastFloatingCursorRect {
            UIView.animate(withDuration: 0.15, animations: { [weak self] in
                if let textInputWrapper = self {
                    textInputWrapper.floatingCursor.frame = CGRect(x: cursorRect.origin.x, y: cursorRect.origin.y, width: textInputWrapper.floatingCursorWidth, height: cursorRect.height + Self.FLOATING_CURSOR_OFFSET_HEIGHT)
                }
            }, completion: {[weak self] finished in
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

        let scrollUp = y >= bounds.height - 20
        let scrollDown = y <= 20
        
        if (scrollUp || scrollDown) && autoScroll == nil {
            autoScroll = Timer.scheduledTimer(withTimeInterval: 0.016, repeats: true) { [self] timer in
                if floatingCursor.isHidden {
                    timer.invalidate()
                }
                
                mouse_moved(wsHandle, Float(bounds.width / 2), Float(bounds.height / 2))
                scroll_wheel(wsHandle, 0, scrollUp ? -20 : 20, false, false, false, false)
                mouse_gone(wsHandle)
                
                mtkView.drawImmediately()
            }
        } else if let autoScroll,
            !scrollUp && !scrollDown {
            autoScroll.invalidate()
            self.autoScroll = nil
        }
        
        if animate {
            UIView.animate(withDuration: 0.15, animations: { [weak self] in
                if let textWrapper = self {
                    textWrapper.floatingCursor.frame = CGRect(x: x, y: y - (cursorRect.height / 2), width: textWrapper.floatingCursorWidth, height: cursorRect.height + Self.FLOATING_CURSOR_OFFSET_HEIGHT)
                }
            })
        } else {
            floatingCursor.frame = CGRect(x: x, y: y - (cursorRect.height / 2), width: floatingCursorWidth, height: cursorRect.height + Self.FLOATING_CURSOR_OFFSET_HEIGHT)
        }
    }
    
    public override func canPerformAction(_ action: Selector, withSender sender: Any?) -> Bool {
        if action == #selector(cut(_:)) || action == #selector(copy(_:)) {
            return selectedTextRange?.isEmpty == false
        }
        
        if action == #selector(paste(_:)) {
            return UIPasteboard.general.hasStrings || UIPasteboard.general.hasImages || UIPasteboard.general.hasURLs
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
              let replacementText = replacement.value(forKey: "replacementText") as? NSString else {
            return
        }
        
        replace(range, withText: replacementText as String)
    }
    
    public func dropInteraction(_ interaction: UIDropInteraction, canHandle session: UIDropSession) -> Bool {
        guard session.items.count == 1 else { return false }

        return session.hasItemsConforming(toTypeIdentifiers: [UTType.image.identifier, UTType.fileURL.identifier, UTType.text.identifier])
    }

    public func dropInteraction(_ interaction: UIDropInteraction, sessionDidUpdate session: UIDropSession) -> UIDropProposal {
        let dropLocation = session.location(in: self)
        let operation: UIDropOperation

        if self.frame.contains(dropLocation) {
            operation = .copy
        } else {
            operation = .cancel
        }

        return UIDropProposal(operation: operation)
    }

    public func dropInteraction(_ interaction: UIDropInteraction, performDrop session: UIDropSession) {
        if session.hasItemsConforming(toTypeIdentifiers: [UTType.image.identifier as String]) {
            session.loadObjects(ofClass: UIImage.self) { imageItems in
                let images = imageItems as? [UIImage] ?? []

                for image in images {
                    self.importContent(.image(image), isPaste: true)
                }
            }
        }

        if session.hasItemsConforming(toTypeIdentifiers: [UTType.text.identifier as String]) {
            session.loadObjects(ofClass: NSAttributedString.self) { textItems in
                let attributedStrings = textItems as? [NSAttributedString] ?? []

                for attributedString in attributedStrings {
                    self.importContent(.text(attributedString.string), isPaste: true)
                }
            }
        }

        if session.hasItemsConforming(toTypeIdentifiers: [UTType.fileURL.identifier as String]) {
            session.loadObjects(ofClass: URL.self) { urlItems in
                for url in urlItems {
                    self.importContent(.url(url), isPaste: true)
                }
            }
        }
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
        guard let _ = (markedTextRange ?? selectedTextRange) as? LBTextRange,
            !text.isEmpty else {
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

            if !floatingCursor.isHidden || isLongPressCursorDrag {
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
        get {
            let range = get_marked(wsHandle)
            if range.none {
                return nil
            }
            return LBTextRange(c: range)
        }
    }

    public var markedTextStyle: [NSAttributedString.Key : Any]? {
        set {
            unimplemented()
        }

        get {
            unimplemented()
            return nil
        }
    }

    public func setMarkedText(_ markedText: String?, selectedRange: NSRange) {
        guard let _ = (markedTextRange ?? selectedTextRange) as? LBTextRange else {
            return
        }
        
        inputDelegate?.textWillChange(self)
        set_marked(wsHandle, CTextRange(none: false, start: CTextPosition(none: false, pos: UInt(selectedRange.lowerBound)), end: CTextPosition(none: false, pos: UInt(selectedRange.upperBound))), markedText)
        mtkView.drawImmediately()
        inputDelegate?.textDidChange(self)
    }

    public func unmarkText() {
        guard let _ = markedTextRange as? LBTextRange else {
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

    public func textRange(from fromPosition: UITextPosition, to toPosition: UITextPosition) -> UITextRange? {
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

    public func position(from position: UITextPosition, in direction: UITextLayoutDirection, offset: Int) -> UITextPosition? {
        let start = (position as! LBTextPos).c
        let direction = CTextLayoutDirection(rawValue: UInt32(direction.rawValue));
        let new = position_offset_in_direction(wsHandle, start, direction, Int32(offset))
        if new.none {
            return nil
        }
        return LBTextPos(c: new)
    }

    public func compare(_ position: UITextPosition, to other: UITextPosition) -> ComparisonResult {
        guard let left = (position as? LBTextPos)?.c.pos, let right = (other as? LBTextPos)?.c.pos else {
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
        
        guard let left = (from as? LBTextPos)?.c.pos, let right = (toPosition as? LBTextPos)?.c.pos else {
            return 0
        }

        return Int(right) - Int(left)
    }

    public var inputDelegate: UITextInputDelegate?

    public lazy var tokenizer: UITextInputTokenizer = LBTokenizer(wsHandle: self.wsHandle)

    public func position(within range: UITextRange, farthestIn direction: UITextLayoutDirection) -> UITextPosition? {
        unimplemented()
        return nil
    }

    public func characterRange(byExtending position: UITextPosition, in direction: UITextLayoutDirection) -> UITextRange? {
        unimplemented()
        return nil
    }

    public func baseWritingDirection(for position: UITextPosition, in direction: UITextStorageDirection) -> NSWritingDirection {
        return NSWritingDirection.leftToRight
    }

    public func setBaseWritingDirection(_ writingDirection: NSWritingDirection, for range: UITextRange) {
        if writingDirection != .leftToRight {
            unimplemented()
        }
    }

    public func firstRect(for range: UITextRange) -> CGRect {
        let range = (range as! LBTextRange).c
        let result = first_rect(wsHandle, range)
        return CGRect(x: result.min_x, y: result.min_y - mtkView.docHeaderSize, width: result.max_x-result.min_x, height: result.max_y-result.min_y)
    }

    public func caretRect(for position: UITextPosition) -> CGRect {
        let position = (position as! LBTextPos).c
        let result = cursor_rect_at_position(wsHandle, position)
        return CGRect(x: result.min_x, y: result.min_y - mtkView.docHeaderSize, width: 1, height:result.max_y - result.min_y)
    }

    public func selectionRects(for range: UITextRange) -> [UITextSelectionRect] {
        let range = (range as! LBTextRange).c
        let result = selection_rects(wsHandle, range)

        let buffer = Array(UnsafeBufferPointer(start: result.rects, count: Int(result.size)))

        free_selection_rects(result)
        
        let selectionRects: [UITextSelectionRect] = buffer.enumerated().map { (index, rect) in
            let new_rect = CRect(min_x: rect.min_x, min_y: rect.min_y - mtkView.docHeaderSize, max_x: rect.max_x, max_y: rect.max_y - mtkView.docHeaderSize)

            return LBTextSelectionRect(cRect: new_rect, loc: index, size: buffer.count)
        }
        
        return selectionRects
    }

    public func closestPosition(to point: CGPoint) -> UITextPosition? {
        let (x, y) = floatingCursor.isHidden ? (point.x, point.y) : (point.x - floatingCursorNewStartX, point.y - floatingCursorNewStartY)

        let point = CPoint(x: x, y: y + mtkView.docHeaderSize)
        let result = position_at_point(wsHandle, point)

        return LBTextPos(c: result)
    }

    public func closestPosition(to point: CGPoint, within range: UITextRange) -> UITextPosition? {
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

    public func deleteBackward() {
        if !hasText {
            return
        }
        
        guard let _ = (markedTextRange ?? selectedTextRange) as? LBTextRange else {
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
            !range.isEmpty else {
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
        let deleteWord = UIKeyCommand(input: UIKeyCommand.inputDelete, modifierFlags: [.alternate], action: #selector(deleteWord))

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

    public override func pressesBegan(_ presses: Set<UIPress>, with event: UIPressesEvent?) {
        mtkView.forwardKeyPress(presses, with: event, pressBegan: true)

        if !mtkView.overrideDefaultKeyboardBehavior {
            super.pressesBegan(presses, with: event)
        }
    }

    public override func pressesEnded(_ presses: Set<UIPress>, with event: UIPressesEvent?) {
        mtkView.forwardKeyPress(presses, with: event, pressBegan: false)

        if !mtkView.overrideDefaultKeyboardBehavior {
            super.pressesEnded(presses, with: event)
        }
    }

    func unimplemented() {
        print("unimplemented!")
        Thread.callStackSymbols.forEach{print($0)}
    }
}

public class iOSMTKDrawingWrapper: UIView, UIPencilInteractionDelegate, UIEditMenuInteractionDelegate {

    public static let TOOL_BAR_HEIGHT: CGFloat = 50

    let pencilInteraction = UIPencilInteraction()
    lazy var editMenuInteraction = UIEditMenuInteraction(delegate: self)
    
    let mtkView: iOSMTK

    var wsHandle: UnsafeMutableRawPointer? { get { mtkView.wsHandle } }
    var workspaceState: WorkspaceState? { get { mtkView.workspaceState } }
    
    var prefersPencilOnlyDrawing: Bool = UIPencilInteraction.prefersPencilOnlyDrawing

    init(mtkView: iOSMTK) {
        self.mtkView = mtkView

        super.init(frame: .infinite)

        isMultipleTouchEnabled = true

        // pen support
        pencilInteraction.delegate = self
        addInteraction(pencilInteraction)
        
        
        // ipad trackpad support
        let pan = UIPanGestureRecognizer(target: self, action: #selector(self.handleTrackpadScroll(_:)))
        pan.allowedScrollTypesMask = .all
        pan.maximumNumberOfTouches = 0
        self.addGestureRecognizer(pan)
        
        // edit menu support
        self.addInteraction(editMenuInteraction)
        
        let pointerInteraction = UIPointerInteraction(delegate: mtkView)
        self.addInteraction(pointerInteraction)
        
        let longPress = UILongPressGestureRecognizer(target: self, action: #selector(self.handleLongPress(_:)))
        longPress.cancelsTouchesInView = false
        self.addGestureRecognizer(longPress)
        
        self.isMultipleTouchEnabled = true
        set_pencil_only_drawing(wsHandle, prefersPencilOnlyDrawing)
    }
    
    @objc private func handleLongPress(_ gesture: UILongPressGestureRecognizer) {
        guard gesture.state == .began else { return }
        let config = UIEditMenuConfiguration(identifier: nil, sourcePoint: gesture.location(in: self))
        editMenuInteraction.presentEditMenu(with: config)
    }
    
    public override func canPerformAction(_ action: Selector, withSender sender: Any?) -> Bool {
        if action == #selector(paste(_:)) {
            return UIPasteboard.general.hasStrings || UIPasteboard.general.hasImages
        }

        return false
    }
    
    @objc func handleTrackpadScroll(_ sender: UIPanGestureRecognizer? = nil) {
        mtkView.handleTrackpadScroll(sender)
    }
    
    @available(iOS 17.5, *)
    public func pencilInteraction(_ interaction: UIPencilInteraction, didReceiveSqueeze squeeze: UIPencilInteraction.Squeeze) {
        if squeeze.phase == .ended {
            show_tool_popover_at_cursor(wsHandle)
        }
    }

    public func pencilInteractionDidTap(_ interaction: UIPencilInteraction) {
        switch UIPencilInteraction.preferredTapAction {
        case .ignore, .showColorPalette, .showInkAttributes:
            print("do nothing")
        case .switchEraser:
            toggle_drawing_tool_between_eraser(wsHandle)
        case .switchPrevious:
            toggle_drawing_tool(wsHandle)
        default:
            print("don't know, do nothing")
        }

        mtkView.setNeedsDisplay(mtkView.frame)
    }
    
    func updateFromPencilState() {
        if UIPencilInteraction.prefersPencilOnlyDrawing != prefersPencilOnlyDrawing {
            prefersPencilOnlyDrawing.toggle()
            set_pencil_only_drawing(wsHandle, prefersPencilOnlyDrawing)
        }
    }

    public override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent?) {
        updateFromPencilState()
        
        mtkView.touchesBegan(touches, with: event)
    }

    public override func touchesMoved(_ touches: Set<UITouch>, with event: UIEvent?) {
        mtkView.touchesMoved(touches, with: event)
    }

    public override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent?) {
        mtkView.touchesEnded(touches, with: event)
    }

    public override func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent?) {
        mtkView.touchesCancelled(touches, with: event)
        
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

public class iOSMTK: MTKView, MTKViewDelegate, UIPointerInteractionDelegate {

    public static let TAB_BAR_HEIGHT: CGFloat = 40
    public static let TITLE_BAR_HEIGHT: CGFloat = 33
    public static let POINTER_DECELERATION_RATE: CGFloat = 0.95

    public var wsHandle: UnsafeMutableRawPointer?
    var workspaceState: WorkspaceState?
    var currentOpenDoc: UUID? = nil
    var currentSelectedFolder: UUID? = nil

    var redrawTask: DispatchWorkItem? = nil

    var tabSwitchTask: (() -> Void)? = nil
    var onSelectionChanged: (() -> Void)? = nil
    var onTextChanged: (() -> Void)? = nil

    weak var currentWrapper: UIView? = nil

    var overrideDefaultKeyboardBehavior = false
    
    var docHeaderSize: Double {
        get {
            !isCompact() ? iOSMTK.TAB_BAR_HEIGHT : 0
        }
    }
    
    var ignoreSelectionUpdate = false
    var ignoreTextUpdate = false
    
    var cursorTracked = false
    var scrollId = 0
        
    override init(frame frameRect: CGRect, device: MTLDevice?) {
        super.init(frame: frameRect, device: device)
        
        let pointerInteraction = UIPointerInteraction(delegate: self)
        self.addInteraction(pointerInteraction)
        
        // ipad trackpad support
        let pan = UIPanGestureRecognizer(target: self, action: #selector(self.handleTrackpadScroll(_:)))
        pan.allowedScrollTypesMask = .all
        pan.maximumNumberOfTouches = 0
        self.addGestureRecognizer(pan)
        
        self.isPaused = false
        self.enableSetNeedsDisplay = false
        self.delegate = self
        self.preferredFramesPerSecond = 120
        self.isUserInteractionEnabled = true
    }

    required init(coder: NSCoder) {
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
                
                if abs(velocity.x) < 0.1 && abs(velocity.y) < 0.1 {
                    timer.invalidate()
                    return
                }
                
                if !cursorTracked {
                    mouse_moved(wsHandle, Float(bounds.width / 2), Float(bounds.height / 2))
                }
                scroll_wheel(self.wsHandle, Float(velocity.x), Float(velocity.y), false, false, false, false)
                if !cursorTracked {
                    mouse_gone(wsHandle)
                }
                
                self.setNeedsDisplay()
            }
        } else {
            if !cursorTracked {
                mouse_moved(wsHandle, Float(bounds.width / 2), Float(bounds.height / 2))
            }
            scroll_wheel(wsHandle, Float(velocity.x), Float(velocity.y), false, false, false, false)
            if !cursorTracked {
                mouse_gone(wsHandle)
            }
        }
        
        self.setNeedsDisplay()
    }

    public func pointerInteraction(_ interaction: UIPointerInteraction, regionFor request: UIPointerRegionRequest, defaultRegion: UIPointerRegion) -> UIPointerRegion? {
        let offsetY: CGFloat = if interaction.view is iOSMTKTextInputWrapper || interaction.view is iOSMTKDrawingWrapper {
            docHeaderSize
        } else {
            0
        }
        
        mouse_moved(wsHandle, Float(request.location.x), Float(request.location.y + offsetY))
        return defaultRegion
    }
    
    public func pointerInteraction(_ interaction: UIPointerInteraction, willEnter region: UIPointerRegion, animator: any UIPointerInteractionAnimating) {
        cursorTracked = true
    }
    
    public func pointerInteraction(_ interaction: UIPointerInteraction, willExit region: UIPointerRegion, animator: any UIPointerInteractionAnimating) {
        cursorTracked = false
        mouse_gone(wsHandle)
    }
    
    func openFile(id: UUID) {
        let uuid = CUuid(_0: id.uuid)
        open_file(wsHandle, uuid, false)
        setNeedsDisplay(self.frame)
    }
    
    func createDocAt(parent: UUID, drawing: Bool) {
        let parent = CUuid(_0: parent.uuid)
        create_doc_at(wsHandle, parent, drawing)
        setNeedsDisplay(self.frame)
    }

    func closeDoc(id: UUID) {
        close_tab(wsHandle, id.uuidString)
        setNeedsDisplay(self.frame)
    }
    
    func closeAllTabs() {
        close_all_tabs(wsHandle)
        setNeedsDisplay(self.frame)
    }
    
    func requestSync() {
        request_sync(wsHandle)
        setNeedsDisplay(self.frame)
    }

    func fileOpCompleted(fileOp: WSFileOpCompleted) {
        switch fileOp {
        case .Delete(let id):
            close_tab(wsHandle, id.uuidString)
            setNeedsDisplay(self.frame)
        case .Rename(let id, let newName):
            tab_renamed(wsHandle, id.uuidString, newName)
            setNeedsDisplay(self.frame)
        }
    }

    public func setInitialContent(_ coreHandle: UnsafeMutableRawPointer?) {
        let metalLayer = UnsafeMutableRawPointer(Unmanaged.passUnretained(self.layer).toOpaque())
        self.wsHandle = init_ws(coreHandle, metalLayer, isDarkMode(), !isCompact())
        workspaceState?.wsHandle = wsHandle
    }
    
    public func mtkView(_ view: MTKView, drawableSizeWillChange size: CGSize) {
        resize_editor(wsHandle, Float(size.width), Float(size.height), Float(self.contentScaleFactor))
        self.setNeedsDisplay()
    }
    
    public func drawImmediately() {
        redrawTask?.cancel()
        redrawTask = nil
        
        ignoreSelectionUpdate = true
        ignoreTextUpdate = true
        
        self.isPaused = true
        self.enableSetNeedsDisplay = false
        
        self.draw(in: self)
        
        ignoreSelectionUpdate = false
        ignoreTextUpdate = false
    }
    
    public func draw(in view: MTKView) {
        if tabSwitchTask != nil {
            tabSwitchTask!()
            tabSwitchTask = nil
        }

        dark_mode(wsHandle, isDarkMode())
        show_hide_tabs(wsHandle, !isCompact())
        set_scale(wsHandle, Float(self.contentScaleFactor))
        
        let output = ios_frame(wsHandle)
        
        if output.status_updated {
            let status = get_status(wsHandle)
            let msg = String(cString: status.msg)
            free_text(status.msg)
            let syncing = status.syncing
            workspaceState?.syncing = syncing
            workspaceState?.statusMsg = msg
        }
        
        if output.tabs_changed {
            workspaceState?.tabCount = Int(tab_count(wsHandle))
        }
        
        workspaceState?.reloadFiles = output.refresh_files

        let selectedFile = UUID(uuid: output.selected_file._0)

        if !selectedFile.isNil() {
            if currentOpenDoc != selectedFile {
                onSelectionChanged?()
                onTextChanged?()
            }

            currentOpenDoc = selectedFile

            if selectedFile != self.workspaceState?.openDoc {
                self.workspaceState?.openDoc = selectedFile
            }
        }

        let currentTab = WorkspaceTab(rawValue: Int(current_tab(wsHandle)))!

        if currentTab != self.workspaceState!.currentTab {
            DispatchQueue.main.async {
                withAnimation {
                    self.workspaceState!.currentTab = currentTab
                }
            }
        }

        if currentTab == .Welcome && currentOpenDoc != nil {
            currentOpenDoc = nil
            self.workspaceState?.openDoc = nil
        }
        
        if let currentWrapper = currentWrapper as? iOSMTKTextInputWrapper,
           currentTab == .Markdown {
            if(output.has_virtual_keyboard_shown && !output.virtual_keyboard_shown && currentWrapper.floatingCursor.isHidden) {
                UIApplication.shared.sendAction(#selector(UIResponder.resignFirstResponder), to: nil, from: nil, for: nil)
            }
            
            if output.scroll_updated {
                onSelectionChanged?()
            }
            
            if output.text_updated && !ignoreTextUpdate {
                onTextChanged?()
            }
            
            if output.selection_updated && !ignoreSelectionUpdate {
                onSelectionChanged?()
            }

            let keyboard_shown = currentWrapper.isFirstResponder && GCKeyboard.coalesced == nil;
            update_virtual_keyboard(wsHandle, keyboard_shown)
        }

        if output.tab_title_clicked {
            workspaceState!.renameOpenDoc = true

            if !isCompact() {
                unfocus_title(wsHandle)
            }
        }

        let newFile = UUID(uuid: output.doc_created._0)
        if !newFile.isNil() {
            openFile(id: newFile)
        }

        if output.new_folder_btn_pressed {
            workspaceState?.newFolderButtonPressed = true
        }

        if let openedUrl = output.url_opened {
            let url = textFromPtr(s: openedUrl)

            if let url = URL(string: url),
                UIApplication.shared.canOpenURL(url) {
                self.workspaceState?.urlOpened = url
            }
        }

        if let text = output.copied_text {
            let text = textFromPtr(s: text)
            if !text.isEmpty {
                UIPasteboard.general.string = text
            }
        }

        redrawTask?.cancel()
        self.isPaused = output.redraw_in > 50
        if self.isPaused {
            let redrawIn = UInt64(truncatingIfNeeded: output.redraw_in)
            let redrawInInterval = DispatchTimeInterval.milliseconds(Int(truncatingIfNeeded: min(500, redrawIn)));

            let newRedrawTask = DispatchWorkItem {
                self.drawImmediately()
            }
            DispatchQueue.main.asyncAfter(deadline: .now() + redrawInInterval, execute: newRedrawTask)
            redrawTask = newRedrawTask
        }
        
        self.enableSetNeedsDisplay = self.isPaused
    }

    override public func traitCollectionDidChange(_ previousTraitCollection: UITraitCollection?) {
        setNeedsDisplay(self.frame)
    }

    public override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent?) {
        for touch in touches {
            let point = Unmanaged.passUnretained(touch).toOpaque()
            let value = UInt64(UInt(bitPattern: point))
            
            for touch in event!.coalescedTouches(for: touch)! {
                let location = touch.preciseLocation(in: self)
                let force = touch.force != 0 ? touch.force / touch.maximumPossibleForce : 0
                touches_began(wsHandle, value, Float(location.x), Float(location.y), Float(force))
            }
        }

        self.setNeedsDisplay(self.frame)
    }

    public override func touchesMoved(_ touches: Set<UITouch>, with event: UIEvent?) {
        for touch in touches {
            let point = Unmanaged.passUnretained(touch).toOpaque()
            let value = UInt64(UInt(bitPattern: point))
                        
            let location = touch.preciseLocation(in: self)
            let force = touch.force != 0 ? touch.force / touch.maximumPossibleForce : 0

            for touch in event!.predictedTouches(for: touch)! {
                let location = touch.preciseLocation(in: self)
                let force = touch.force != 0 ? touch.force / touch.maximumPossibleForce : 0
                touches_predicted(wsHandle, value, Float(location.x), Float(location.y), Float(force))
            }

            touches_moved(wsHandle, value, Float(location.x), Float(location.y), Float(force))

        }

        self.setNeedsDisplay(self.frame)
    }

    public override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent?) {
        for touch in touches {
            let point = Unmanaged.passUnretained(touch).toOpaque()
            let value = UInt64(UInt(bitPattern: point))

            let location = touch.preciseLocation(in: self)
            let force = touch.force != 0 ? touch.force / touch.maximumPossibleForce : 0
            touches_ended(wsHandle, value, Float(location.x), Float(location.y), Float(force))
        }

        self.setNeedsDisplay(self.frame)
    }

    public override func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent?) {
        for touch in touches {
            let point = Unmanaged.passUnretained(touch).toOpaque()
            let value = UInt64(UInt(bitPattern: point))

            let location = touch.preciseLocation(in: self)
            let force = touch.force != 0 ? touch.force / touch.maximumPossibleForce : 0
            
            touches_cancelled(wsHandle, value, Float(location.x), Float(location.y), Float(force))
                        
        }

        self.setNeedsDisplay(self.frame)
    }

    public override func pressesBegan(_ presses: Set<UIPress>, with event: UIPressesEvent?) {
        forwardKeyPress(presses, with: event, pressBegan: true)

        if !overrideDefaultKeyboardBehavior {
            super.pressesBegan(presses, with: event)
        }
    }

    public override func pressesEnded(_ presses: Set<UIPress>, with event: UIPressesEvent?) {
        forwardKeyPress(presses, with: event, pressBegan: false)

        if !overrideDefaultKeyboardBehavior {
            super.pressesEnded(presses, with: event)
        }
    }
    
    func forwardKeyPress(_ presses: Set<UIPress>, with event: UIPressesEvent?, pressBegan: Bool) {
        overrideDefaultKeyboardBehavior = false

        for press in presses {
            guard let key = press.key else { continue }

            if workspaceState!.currentTab.isTextEdit() && key.keyCode == .keyboardDeleteOrBackspace {
                return
            }

            let shift = key.modifierFlags.contains(.shift)
            let ctrl = key.modifierFlags.contains(.control)
            let option = key.modifierFlags.contains(.alternate)
            let command = key.modifierFlags.contains(.command)

            if (command && key.keyCode == .keyboardW) || (shift && key.keyCode == .keyboardTab) {
                overrideDefaultKeyboardBehavior = true
            }

            ios_key_event(wsHandle, key.keyCode.rawValue, shift, ctrl, option, command, pressBegan)
            self.setNeedsDisplay(self.frame)
        }
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
            workspaceState?.pasted = true
        }
    }
    
    func sendImage(img: Data, isPaste: Bool) {
        let imgPtr = img.withUnsafeBytes { (pointer: UnsafeRawBufferPointer) -> UnsafePointer<UInt8> in
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
        Thread.callStackSymbols.forEach{print($0)}
//        exit(-69)
    }

    public override var canBecomeFocused: Bool {
        return true
    }
}

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

class LBTokenizer: NSObject, UITextInputTokenizer {
    let wsHandle: UnsafeMutableRawPointer?

    init(wsHandle: UnsafeMutableRawPointer?) {
        self.wsHandle = wsHandle
    }

    func isPosition(_ position: UITextPosition, atBoundary granularity: UITextGranularity, inDirection direction: UITextDirection) -> Bool {
        guard let position = (position as? LBTextPos)?.c else {
            return false
        }
        let granularity = CTextGranularity(rawValue: UInt32(granularity.rawValue))
        let backwards = direction.rawValue == UITextStorageDirection.backward.rawValue
        return is_position_at_bound(wsHandle, position, granularity, backwards)
    }

    func isPosition(_ position: UITextPosition, withinTextUnit granularity: UITextGranularity, inDirection direction: UITextDirection) -> Bool {
        guard let position = (position as? LBTextPos)?.c else {
            return false
        }
        let granularity = CTextGranularity(rawValue: UInt32(granularity.rawValue))
        let backwards = direction.rawValue == UITextStorageDirection.backward.rawValue
        return is_position_within_bound(wsHandle, position, granularity, backwards)
    }

    func position(from position: UITextPosition, toBoundary granularity: UITextGranularity, inDirection direction: UITextDirection) -> UITextPosition? {
        guard let position = (position as? LBTextPos)?.c else {
            return nil
        }
        let granularity = CTextGranularity(rawValue: UInt32(granularity.rawValue))
        let backwards = direction.rawValue == UITextStorageDirection.backward.rawValue
        let result = bound_from_position(wsHandle, position, granularity, backwards)
        return LBTextPos(c: result)
    }

    func rangeEnclosingPosition(_ position: UITextPosition, with granularity: UITextGranularity, inDirection direction: UITextDirection) -> UITextRange? {
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

class iOSUndoManager: UndoManager {

    public var wsHandle: UnsafeMutableRawPointer? = nil
    
    weak var textWrapper: iOSMTKTextInputWrapper? = nil
    weak var mtkView: iOSMTK? = nil
    weak var inputDelegate: UITextInputDelegate? = nil
    
    override var canUndo: Bool {
        get {
            can_undo(wsHandle)
        }
    }

    override var canRedo: Bool {
        get {
            can_redo(wsHandle)
        }
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

public enum SupportedImportFormat {
    case url(URL)
    case image(UIImage)
    case text(String)
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
        get {
            return .leftToRight
        }
    }
    override var containsStart: Bool {
        get {
            return loc == 0
        }
    }
    override var containsEnd: Bool {
        get {
            return loc == (size - 1)
        }
    }
    override var isVertical: Bool {
        get {
            return false
        }
    }

    override var rect: CGRect {
        get {
            return CGRect(x: cRect.min_x, y: cRect.min_y, width: cRect.max_x - cRect.min_x, height: cRect.max_y - cRect.min_y)
        }
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
