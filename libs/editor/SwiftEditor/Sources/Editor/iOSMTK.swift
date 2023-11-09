#if os(iOS)
import UIKit
import MetalKit
import Bridge
import SwiftUI
import MobileCoreServices
import UniformTypeIdentifiers

public class iOSMTK: MTKView, MTKViewDelegate, UITextInput, UIEditMenuInteractionDelegate, UIDropInteractionDelegate {
    
    var editorHandle: UnsafeMutableRawPointer?
    var editorState: EditorState?
    var toolbarState: ToolbarState?
    var nameState: NameState?
    var hasSelection: Bool = false
    
    var textUndoManager = iOSUndoManager()

    var redrawTask: DispatchWorkItem? = nil

    public override var undoManager: UndoManager? {
        return textUndoManager
    }

    var pasteBoardEventId: Int = 0
    var lastKnownTapLocation: Float? = nil
    override init(frame frameRect: CGRect, device: MTLDevice?) {
        super.init(frame: frameRect, device: device)
        
        self.isPaused = true
        self.enableSetNeedsDisplay = true
        self.delegate = self
        self.preferredFramesPerSecond = 120

        // regain focus on tap
        let tap = UITapGestureRecognizer(target: self, action: #selector(self.handleTap(_:)))
        tap.cancelsTouchesInView = false
        self.addGestureRecognizer(tap)

        // ipad trackpad support
        let pan = UIPanGestureRecognizer(target: self, action: #selector(self.handleTrackpadScroll(_:)))
        pan.allowedScrollTypesMask = .all
        pan.maximumNumberOfTouches  = 0
        self.addGestureRecognizer(pan)

        // drop support
        let dropInteraction = UIDropInteraction(delegate: self)
        self.addInteraction(dropInteraction)
    }

    @objc func handleTap(_ sender: UITapGestureRecognizer) {
        if sender.state == .ended {
            becomeFirstResponder()
        }
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
                    let _ = self.importContent(.image(image))
                }
            }
        }

        if session.hasItemsConforming(toTypeIdentifiers: [UTType.text.identifier as String]) {
            session.loadObjects(ofClass: NSAttributedString.self) { textItems in
                let attributedStrings = textItems as? [NSAttributedString] ?? []

                for attributedString in attributedStrings {
                    let _ = self.importContent(.text(attributedString.string))
                }
            }
        }

        if session.hasItemsConforming(toTypeIdentifiers: [UTType.fileURL.identifier as String]) {
            session.loadObjects(ofClass: URL.self) { urlItems in
                for url in urlItems {
                    let _ = self.importContent(.url(url))
                }
            }
        }
    }
    
    @objc func handleTrackpadScroll(_ sender: UIPanGestureRecognizer? = nil) {
        if let event = sender {
            
            if event.state == .ended || event.state == .cancelled || event.state == .failed {
                // todo: evaluate fling when desired
                lastKnownTapLocation = nil
                return
            }
            
            let location = event.translation(in: self)
            
            let y = Float(location.y)
            
            if lastKnownTapLocation == nil {
                lastKnownTapLocation = y
            }
            scroll_wheel(editorHandle, y - lastKnownTapLocation!)
            
            lastKnownTapLocation = y
            self.setNeedsDisplay()
        }
        
    }

    public func header(headingSize: UInt32) {
        inputDelegate?.textWillChange(self)
        apply_style_to_selection_header(editorHandle, headingSize)
        self.setNeedsDisplay(self.frame)
    }
    
    public func bulletedList() {
        inputDelegate?.textWillChange(self)
        apply_style_to_selection_bulleted_list(editorHandle)
        self.setNeedsDisplay(self.frame)
    }
    
    public func numberedList() {
        inputDelegate?.textWillChange(self)
        apply_style_to_selection_numbered_list(editorHandle)
        self.setNeedsDisplay(self.frame)
    }
    
    public func todoList() {
        inputDelegate?.textWillChange(self)
        apply_style_to_selection_todo_list(editorHandle)
        self.setNeedsDisplay(self.frame)
    }
    
    public func bold() {
        inputDelegate?.textWillChange(self)
        apply_style_to_selection_bold(editorHandle)
        self.setNeedsDisplay(self.frame)
    }
    
    public func italic() {
        inputDelegate?.textWillChange(self)
        apply_style_to_selection_italic(editorHandle)
        self.setNeedsDisplay(self.frame)
    }
    
    public func inlineCode() {
        inputDelegate?.textWillChange(self)
        apply_style_to_selection_inline_code(editorHandle)
        self.setNeedsDisplay(self.frame)
    }
    
    public func strikethrough() {
        inputDelegate?.textWillChange(self)
        apply_style_to_selection_strikethrough(editorHandle)
        self.setNeedsDisplay(self.frame)
    }
    
    public func tab(deindent: Bool) {
        inputDelegate?.textWillChange(self)
        indent_at_cursor(editorHandle, deindent)
        self.setNeedsDisplay(self.frame)
    }

    // used for shortcut
    @objc public func deindent() {
        tab(deindent: true)
    }

    func importContent(_ importFormat: SupportedImportFormat) -> Bool {
        switch importFormat {
        case .url(let url):
            if let markdownURL = editorState!.importFile(url) {
                paste_text(editorHandle, markdownURL)
                editorState?.pasted = true

                return true
            }
        case .image(let image):
            if let data = image.pngData() ?? image.jpegData(compressionQuality: 1.0),
               let url = createTempDir() {
                let imageUrl = url.appendingPathComponent(String(UUID().uuidString.prefix(10).lowercased()), conformingTo: .png)

                do {
                    try data.write(to: imageUrl)
                } catch {
                    return false
                }

                if let lbImageURL = editorState!.importFile(imageUrl) {
                    paste_text(editorHandle, lbImageURL)
                    editorState?.pasted = true

                    return true
                }
            }
        case .text(let text):
            paste_text(editorHandle, text)
            editorState?.pasted = true

            return true
        }

        return false
    }

    public func setInitialContent(_ coreHandle: UnsafeMutableRawPointer?, _ s: String) {
        let metalLayer = UnsafeMutableRawPointer(Unmanaged.passUnretained(self.layer).toOpaque())
        self.editorHandle = init_editor(coreHandle, metalLayer, s, isDarkMode())
        self.textUndoManager.editorHandle = self.editorHandle
        self.textUndoManager.onUndoRedo = {
            self.setNeedsDisplay(self.frame)
        }
        
        self.toolbarState!.toggleBold = bold
        self.toolbarState!.toggleItalic = italic
        self.toolbarState!.toggleTodoList = todoList
        self.toolbarState!.toggleBulletList = bulletedList
        self.toolbarState!.toggleInlineCode = inlineCode
        self.toolbarState!.toggleStrikethrough = strikethrough
        self.toolbarState!.toggleNumberList = numberedList
        self.toolbarState!.toggleHeading = header
        self.toolbarState!.tab = tab
        self.toolbarState!.undoRedo = undoRedo
    }
    
    public func mtkView(_ view: MTKView, drawableSizeWillChange size: CGSize) {
        resize_editor(editorHandle, Float(size.width), Float(size.height), Float(self.contentScaleFactor))
        self.setNeedsDisplay()
    }
    
    public func draw(in view: MTKView) {
        dark_mode(editorHandle, isDarkMode())
        set_scale(editorHandle, Float(self.contentScaleFactor))
        let output = draw_editor(editorHandle)
        
        toolbarState?.isHeadingSelected = output.editor_response.cursor_in_heading;
        toolbarState?.isTodoListSelected = output.editor_response.cursor_in_todo_list;
        toolbarState?.isBulletListSelected = output.editor_response.cursor_in_bullet_list;
        toolbarState?.isNumberListSelected = output.editor_response.cursor_in_number_list;
        toolbarState?.isInlineCodeSelected = output.editor_response.cursor_in_inline_code;
        toolbarState?.isBoldSelected = output.editor_response.cursor_in_bold;
        toolbarState?.isItalicSelected = output.editor_response.cursor_in_italic;
        toolbarState?.isStrikethroughSelected = output.editor_response.cursor_in_strikethrough;
        
        if let potentialTitle = output.editor_response.potential_title {
            nameState?.potentialTitle = String(cString: potentialTitle)
            free_text(UnsafeMutablePointer(mutating: potentialTitle))
        } else {
            nameState?.potentialTitle = nil
        }
        
        if output.editor_response.selection_updated {
            inputDelegate?.selectionDidChange(self)
        }

        if output.editor_response.text_updated {
            inputDelegate?.textDidChange(self)
        }

        if output.editor_response.scroll_updated {
            inputDelegate?.selectionDidChange(self)
        }

        if let openedURLSeq = output.editor_response.opened_url {
            let openedURL = String(cString: openedURLSeq)
            free_text(UnsafeMutablePointer(mutating: openedURLSeq))
            
            if let url = URL(string: openedURL),
                UIApplication.shared.canOpenURL(url) {
                
                UIApplication.shared.open(url)
            }
        }
        
        if output.editor_response.text_updated {
            self.textChanged()
        }
        
        if has_copied_text(editorHandle) {
            UIPasteboard.general.string = getCoppiedText()
        }

        redrawTask?.cancel()
        self.isPaused = output.redraw_in > 100
        if self.isPaused {
            let newRedrawTask = DispatchWorkItem {
                self.setNeedsDisplay(self.frame)
            }
            DispatchQueue.main.asyncAfter(deadline: .now() + .milliseconds(Int(truncatingIfNeeded: output.redraw_in)), execute: newRedrawTask)
            redrawTask = newRedrawTask
        }
    }
    
    func getCoppiedText() -> String {
        let result = get_copied_text(editorHandle)
        let str = String(cString: result!)
        free_text(UnsafeMutablePointer(mutating: result))
        return str
    }
    
    func setClipboard(){
        let pasteboardString: String? = UIPasteboard.general.string
        if let theString = pasteboardString {
            system_clipboard_changed(editorHandle, theString)
        }
        self.pasteBoardEventId = UIPasteboard.general.changeCount
    }
    
    public func insertText(_ text: String) {
        inputDelegate?.textWillChange(self)
        insert_text(editorHandle, text)
        self.setNeedsDisplay(self.frame)
    }
    
    public func text(in range: UITextRange) -> String? {
        let range = (range as! LBTextRange).c
        let result = text_in_range(editorHandle, range)
        let str = String(cString: result!)
        free_text(UnsafeMutablePointer(mutating: result))
        return str
    }
    
    
    public func replace(_ range: UITextRange, withText text: String) {
        let range = range as! LBTextRange
        inputDelegate?.textWillChange(self)
        replace_text(editorHandle, range.c, text)
        self.setNeedsDisplay(self.frame)
    }
    
    public var selectedTextRange: UITextRange? {
        set {
            let range = (newValue as! LBTextRange).c
            inputDelegate?.selectionWillChange(self)
            set_selected(editorHandle, range)
            self.setNeedsDisplay()
        }
        
        get {
            let range = get_selected(editorHandle)
            if range.none {
                return nil
            }
            return LBTextRange(c: range)
        }
    }
    
    public var markedTextRange: UITextRange? {
        get {
            let range = get_marked(editorHandle)
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
        inputDelegate?.textWillChange(self)
        set_marked(editorHandle, CTextRange(none: false, start: CTextPosition(none: false, pos: UInt(selectedRange.lowerBound)), end: CTextPosition(none: false, pos: UInt(selectedRange.upperBound))), markedText)
        self.setNeedsDisplay()
    }
    
    public func unmarkText() {
        inputDelegate?.textWillChange(self)
        unmark_text(editorHandle)
        self.setNeedsDisplay()
    }
    
    public var beginningOfDocument: UITextPosition {
        let res = beginning_of_document(editorHandle)
        return LBTextPos(c: res)
    }
    
    public var endOfDocument: UITextPosition {
        let res = end_of_document(editorHandle)
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
        let new = position_offset(editorHandle, start, Int32(offset))
        if new.none {
            return nil
        }
        return LBTextPos(c: new)
    }
    
    public func position(from position: UITextPosition, in direction: UITextLayoutDirection, offset: Int) -> UITextPosition? {
        let start = (position as! LBTextPos).c
        let direction = CTextLayoutDirection(rawValue: UInt32(direction.rawValue));
        let new = position_offset_in_direction(editorHandle, start, direction, Int32(offset))
        if new.none {
            return nil
        }
        return LBTextPos(c: new)
    }
    
    public func compare(_ position: UITextPosition, to other: UITextPosition) -> ComparisonResult {
        let left = (position as! LBTextPos).c.pos
        let right = (other as! LBTextPos).c.pos
        
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
        
        print("getting offset from \(right) - \(left) getting \(Int(right) - Int(left))")
        
        return Int(right) - Int(left)
    }
    
    public var inputDelegate: UITextInputDelegate?
    
    public lazy var tokenizer: UITextInputTokenizer = LBTokenizer(editorHandle: self.editorHandle)
    
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
        print("first rect")
        let range = (range as! LBTextRange).c
        let result = first_rect(editorHandle, range)
        return CGRect(x: result.min_x, y: result.min_y, width: result.max_x-result.min_x, height: result.max_y-result.min_y)
    }
    
    public func caretRect(for position: UITextPosition) -> CGRect {
        let position = (position as! LBTextPos).c
        let result = cursor_rect_at_position(editorHandle, position)
        print("caret rect \(position.pos)")
        return CGRect(x: result.min_x, y: result.min_y, width: 1, height: result.max_y-result.min_y)
    }
    
    public func selectionRects(for range: UITextRange) -> [UITextSelectionRect] {
        print("selection rect")
        let range = (range as! LBTextRange).c
        let result = selection_rects(editorHandle, range)
        let buffer = Array(UnsafeBufferPointer(start: result.rects, count: Int(result.size)))

        return buffer.enumerated().map { (index, rect) in
            return LBTextSelectionRect(cRect: rect, loc: index, size: buffer.count)
        }
    }
    
    public func closestPosition(to point: CGPoint) -> UITextPosition? {
        let point = CPoint(x: point.x, y: point.y)
        let result = position_at_point(editorHandle, point)
        print("closest pos \(LBTextPos(c: result).c.pos)")
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
        let res = has_text(editorHandle)
        return res
    }
    
    public func deleteBackward() {
        inputDelegate?.textWillChange(self)
        backspace(editorHandle)
        
        print("deleting backward")
        
        textChanged()
        self.setNeedsDisplay(self.frame)
    }
    
    public override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent?) {
        let point = Unmanaged.passUnretained(touches.first!).toOpaque()
        let value = UInt64(UInt(bitPattern: point))
        let location = touches.first!.location(in: self)
        touches_began(editorHandle, value, Float(location.x), Float(location.y), Float(touches.first?.force ?? 0))

        self.setNeedsDisplay(self.frame)
    }
    
    public override func touchesMoved(_ touches: Set<UITouch>, with event: UIEvent?) {
        let point = Unmanaged.passUnretained(touches.first!).toOpaque()
        let value = UInt64(UInt(bitPattern: point))
        let location = touches.first!.location(in: self)

        inputDelegate?.selectionWillChange(self)

        touches_moved(editorHandle, value, Float(location.x), Float(location.y), Float(touches.first?.force ?? 0))
        self.setNeedsDisplay(self.frame)
    }
    
    public override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent?) {
        let point = Unmanaged.passUnretained(touches.first!).toOpaque()
        let value = UInt64(UInt(bitPattern: point))
        let location = touches.first!.location(in: self)
        touches_ended(editorHandle, value, Float(location.x), Float(location.y), Float(touches.first?.force ?? 0))
        self.setNeedsDisplay(self.frame)
    }
    
    
    public override func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent?) {
        let point = Unmanaged.passUnretained(touches.first!).toOpaque()
        let value = UInt64(UInt(bitPattern: point))
        let location = touches.first!.location(in: self)
        touches_cancelled(editorHandle, value, Float(location.x), Float(location.y), Float(touches.first?.force ?? 0))
        self.setNeedsDisplay(self.frame)
    }
    
    public func editMenu(for textRange: UITextRange, suggestedActions: [UIMenuElement]) -> UIMenu? {
        let customMenu = self.selectedTextRange?.isEmpty == false ? UIMenu(title: "", options: .displayInline, children: [
            UIAction(title: "Cut") { _ in
                self.inputDelegate?.selectionWillChange(self)
                self.clipboardCut()
            },
            UIAction(title: "Copy") { _ in
                self.clipboardCopy()
            },
            UIAction(title: "Paste") { _ in
                self.inputDelegate?.textWillChange(self)
                self.clipboardPaste()
            },
            UIAction(title: "Select All") { _ in
                self.inputDelegate?.selectionWillChange(self)
                select_all(self.editorHandle)
                self.setNeedsDisplay(self.frame)
            },
        ]) : UIMenu(title: "", options: .displayInline, children: [
            UIAction(title: "Select") { _ in
                self.inputDelegate?.selectionWillChange(self)
                select_current_word(self.editorHandle)
                self.setNeedsDisplay(self.frame)
            },
            UIAction(title: "Select All") { _ in
                self.inputDelegate?.selectionWillChange(self)
                select_all(self.editorHandle)
                self.setNeedsDisplay(self.frame)
            },
            UIAction(title: "Paste") { _ in
                self.inputDelegate?.textWillChange(self)
                self.clipboardPaste()
            },
        ])
        
        var actions = suggestedActions
        actions.append(customMenu)
        return UIMenu(children: actions)
    }
    
    @objc func clipboardCopy() {
        clipboard_copy(self.editorHandle)
        self.setNeedsDisplay(self.frame)
    }
    
    @objc func clipboardCut() {
        // maybe selection will change?
        inputDelegate?.textWillChange(self)
        clipboard_cut(self.editorHandle)
        self.setNeedsDisplay(self.frame)
    }
    
    @objc func clipboardPaste() {
        self.setClipboard()
        if let image = UIPasteboard.general.image {
            if importContent(.image(image)) {
                return
            }
        }

        inputDelegate?.textWillChange(self)
        clipboard_paste(self.editorHandle)
        self.setNeedsDisplay()
    }
    
    @objc func keyboardSelectAll() {
        inputDelegate?.selectionWillChange(self)
        select_all(self.editorHandle)
        self.setNeedsDisplay()
    }

    func undoRedo(redo: Bool) {
        inputDelegate?.textWillChange(self)
        undo_redo(self.editorHandle, redo)
        self.setNeedsDisplay(self.frame)
    }

    func updateText(_ s: String) {
        inputDelegate?.textWillChange(self)
        set_text(editorHandle, s)
        setNeedsDisplay(self.frame)
    }
    
    func textChanged() {
        self.editorState?.text = getText()
    }
    
    func getText() -> String {
        let result = get_text(editorHandle)
        let str = String(cString: result!)
        free_text(UnsafeMutablePointer(mutating: result))
        return str
    }
    
    public override var canBecomeFirstResponder: Bool {
        return true
    }
    
    override public func traitCollectionDidChange(_ previousTraitCollection: UITraitCollection?) {
        dark_mode(editorHandle, isDarkMode())
        setNeedsDisplay(self.frame)
    }
    
    func isDarkMode() -> Bool {
        traitCollection.userInterfaceStyle != .light
    }
    
    override public var keyCommands: [UIKeyCommand]? {
        let deindent = UIKeyCommand(input: "\t", modifierFlags: .shift, action: #selector(deindent))

        deindent.wantsPriorityOverSystemBehavior = true

        return [
            UIKeyCommand(input: "c", modifierFlags: .command, action: #selector(clipboardCopy)),
            UIKeyCommand(input: "x", modifierFlags: .command, action: #selector(clipboardCut)),
            UIKeyCommand(input: "v", modifierFlags: .command, action: #selector(clipboardPaste)),
            UIKeyCommand(input: "a", modifierFlags: .command, action: #selector(keyboardSelectAll)),
            deindent,
        ]
    }
    
    deinit {
        deinit_editor(editorHandle)
    }
    
    @objc func deleteWord() {
        delete_word(editorHandle)
        setNeedsDisplay(self.frame)
    }

    required init(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }
    
    func unimplemented() {
        print("unimplemented!")
        Thread.callStackSymbols.forEach{print($0)}
//        exit(-69)
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
    let editorHandle: UnsafeMutableRawPointer?
    
    init(editorHandle: UnsafeMutableRawPointer?) {
        self.editorHandle = editorHandle
    }
    
    func isPosition(_ position: UITextPosition, atBoundary granularity: UITextGranularity, inDirection direction: UITextDirection) -> Bool {
        let position = (position as! LBTextPos).c
        let granularity = CTextGranularity(rawValue: UInt32(granularity.rawValue))
        let backwards = direction.rawValue == 1
        return is_position_at_bound(editorHandle, position, granularity, backwards)
    }
    
    func isPosition(_ position: UITextPosition, withinTextUnit granularity: UITextGranularity, inDirection direction: UITextDirection) -> Bool {
        let position = (position as! LBTextPos).c
        let granularity = CTextGranularity(rawValue: UInt32(granularity.rawValue))
        let backwards = direction.rawValue == 1
        return is_position_within_bound(editorHandle, position, granularity, backwards)
    }
    
    func position(from position: UITextPosition, toBoundary granularity: UITextGranularity, inDirection direction: UITextDirection) -> UITextPosition? {
        let position = (position as! LBTextPos).c
        let granularity = CTextGranularity(rawValue: UInt32(granularity.rawValue))
        let backwards = direction.rawValue == 1
        let result = bound_from_position(editorHandle, position, granularity, backwards)
        return LBTextPos(c: result)
    }
    
    func rangeEnclosingPosition(_ position: UITextPosition, with granularity: UITextGranularity, inDirection direction: UITextDirection) -> UITextRange? {
        let position = (position as! LBTextPos).c
        let granularity = CTextGranularity(rawValue: UInt32(granularity.rawValue))
        let backwards = direction.rawValue == 1
        let result = bound_at_position(editorHandle, position, granularity, backwards)
        return LBTextRange(c: result)
    }
}

class iOSUndoManager: UndoManager {

    public var editorHandle: UnsafeMutableRawPointer? = nil
    var onUndoRedo: (() -> Void)? = nil

    override var canUndo: Bool {
        get {
            can_undo(editorHandle)
        }
    }

    override var canRedo: Bool {
        get {
            can_redo(editorHandle)
        }
    }

    override func undo() {
        undo_redo(editorHandle, false)
        onUndoRedo?()
    }

    override func redo() {
        undo_redo(editorHandle, true)
        onUndoRedo?()
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
    let cRect: CRect

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
