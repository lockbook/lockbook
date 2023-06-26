#if os(iOS)
import UIKit
import MetalKit
import Bridge
import SwiftUI

public class iOSMTK: MTKView, MTKViewDelegate, UITextInput, UIEditMenuInteractionDelegate {
    
    var editorHandle: UnsafeMutableRawPointer?
    var editorState: EditorState?
    var editMenuInteraction: UIEditMenuInteraction?
    var hasSelection: Bool = false

    var pasteBoardEventId: Int = 0
    var lastKnownTapLocation: Float? = nil
    override init(frame frameRect: CGRect, device: MTLDevice?) {
        super.init(frame: frameRect, device: device)
        
        self.isPaused = false
        self.enableSetNeedsDisplay = true
        self.delegate = self
        self.editMenuInteraction = UIEditMenuInteraction(delegate: self)
        self.addInteraction(self.editMenuInteraction!)
        self.preferredFramesPerSecond = 120
        
        // ipad trackpad support
        let tap = UIPanGestureRecognizer(target: self, action: #selector(self.handleTrackpadScroll(_:)))
        tap.allowedScrollTypesMask = .all
        tap.maximumNumberOfTouches  = 0
        self.addGestureRecognizer(tap)
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
        apply_style_to_selection_header(editorHandle, headingSize)
        self.setNeedsDisplay(self.frame)
    }
    
    public func bulletedList() {
        apply_style_to_selection_bulleted_list(editorHandle)
        self.setNeedsDisplay(self.frame)
    }
    
    public func numberedList() {
        apply_style_to_selection_numbered_list(editorHandle)
        self.setNeedsDisplay(self.frame)
    }
    
    public func todoList() {
        apply_style_to_selection_todo_list(editorHandle)
        self.setNeedsDisplay(self.frame)
    }
    
    public func bold() {
        apply_style_to_selection_bold(editorHandle)
        self.setNeedsDisplay(self.frame)
    }
    
    public func italic() {
        apply_style_to_selection_italic(editorHandle)
        self.setNeedsDisplay(self.frame)
    }
    
    public func inlineCode() {
        apply_style_to_selection_inline_code(editorHandle)
        self.setNeedsDisplay(self.frame)
    }
    
    public func tab(deindent: Bool) {
        indent_at_cursor(editorHandle, deindent)
        self.setNeedsDisplay(self.frame)
    }
    
    public func automaticTitleComputation(_ computeTitle: Bool) {
        set_automatic_title_computation(editorHandle, computeTitle)
    }
    
    public func setInitialContent(_ s: String) {
        let metalLayer = UnsafeMutableRawPointer(Unmanaged.passUnretained(self.layer).toOpaque())
        self.editorHandle = init_editor(metalLayer, s, isDarkMode())
    }
    
    public func mtkView(_ view: MTKView, drawableSizeWillChange size: CGSize) {
        resize_editor(editorHandle, Float(size.width), Float(size.height), Float(self.contentScaleFactor))
        self.setNeedsDisplay()
    }
    
    public func draw(in view: MTKView) {
        dark_mode(editorHandle, isDarkMode())
        set_scale(editorHandle, Float(self.contentScaleFactor))
        let output = draw_editor(editorHandle)
        self.isPaused = !output.redraw
        
        editorState?.isHeadingSelected = output.editor_response.cursor_in_heading;
        editorState?.isTodoListSelected = output.editor_response.cursor_in_todo_list;
        editorState?.isBulletListSelected = output.editor_response.cursor_in_bullet_list;
        editorState?.isNumberListSelected = output.editor_response.cursor_in_number_list;
        editorState?.isInlineCodeSelected = output.editor_response.cursor_in_inline_code;
        editorState?.isBoldSelected = output.editor_response.cursor_in_bold;
        editorState?.isItalicSelected = output.editor_response.cursor_in_italic;
        
        if let potentialTitle = output.editor_response.potential_title {
            editorState?.potentialTitle = String(cString: potential_title)
        } else {
            editorState?.potentialTitle = nil
        }
        
        if output.editor_response.show_edit_menu {
            self.hasSelection = output.editor_response.has_selection
            let location = CGPoint(
                x: Double(output.editor_response.edit_menu_x),
                y: Double(output.editor_response.edit_menu_y)
            )
            
            let configuration = UIEditMenuConfiguration(identifier: nil, sourcePoint: location)
            if let interaction = editMenuInteraction {
                interaction.presentEditMenu(with: configuration)
            }
        }
        
        if output.editor_response.text_updated {
            self.textChanged()
        }
        
        if has_copied_text(editorHandle) {
            UIPasteboard.general.string = getCoppiedText()
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
            print("clipboard contents: \(theString)")
            system_clipboard_changed(editorHandle, theString)
        }
        self.pasteBoardEventId = UIPasteboard.general.changeCount
    }
    
    public func insertText(_ text: String) {
        //        print("\(#function)(\(text))")
        insert_text(editorHandle, text)
        self.setNeedsDisplay(self.frame)
    }
    
    public func text(in range: UITextRange) -> String? {
        let range = (range as! LBTextRange).c
        let result = text_in_range(editorHandle, range)
        let str = String(cString: result!)
        free_text(UnsafeMutablePointer(mutating: result))
        //        print("\(#function)(\(range)) -> ...")
        return str
    }
    
    
    public func replace(_ range: UITextRange, withText text: String) {
        print("\(#function)(\(range), \(text)")
        let range = range as! LBTextRange
        replace_text(editorHandle, range.c, text)
        self.setNeedsDisplay(self.frame)
    }
    
    public var selectedTextRange: UITextRange? {
        set {
            let range = (newValue as! LBTextRange).c
            print("set \(#function) = \(range)")
            set_selected(editorHandle, range)
            self.setNeedsDisplay()
        }
        
        get {
            let range = get_selected(editorHandle)
            if range.none {
                //                print("get \(#function) -> nil")
                return nil
            } else {
                //                print("get \(#function) -> \(range)")
            }
            return LBTextRange(c: range)
        }
    }
    
    public var markedTextRange: UITextRange? {
        get {
            let range = get_marked(editorHandle)
            if range.none {
                //                print("get \(#function) -> nil")
                return nil
            } else {
                //                print("get \(#function) -> \(range)")
            }
            return LBTextRange(c: range)
        }
    }
    
    public var markedTextStyle: [NSAttributedString.Key : Any]? {
        set {
            print("set \(#function)")
            unimplemented()
        }
        
        get {
            print("get \(#function)")
            unimplemented()
            return nil
        }
    }
    
    public func setMarkedText(_ markedText: String?, selectedRange: NSRange) {
        set_marked(editorHandle, CTextRange(none: false, start: CTextPosition(none: false, pos: UInt(selectedRange.lowerBound)), end: CTextPosition(none: false, pos: UInt(selectedRange.upperBound))), markedText)
        self.setNeedsDisplay()
    }
    
    public func unmarkText() {
        unmark_text(editorHandle)
        self.setNeedsDisplay()
    }
    
    public var beginningOfDocument: UITextPosition {
        let res = beginning_of_document(editorHandle)
        //        print("\(#function) -> \(res)")
        return LBTextPos(c: res)
    }
    
    public var endOfDocument: UITextPosition {
        let res = end_of_document(editorHandle)
        //        print("\(#function) -> \(res)")
        return LBTextPos(c: res)
    }
    
    public func textRange(from fromPosition: UITextPosition, to toPosition: UITextPosition) -> UITextRange? {
        let start = (fromPosition as! LBTextPos).c
        let end = (toPosition as! LBTextPos).c
        let range = text_range(start, end)
        if range.none {
            //            print("\(#function)(\(start), \(end) -> nil")
            return nil
        } else {
            //            print("\(#function)(\(start), \(end) -> \(range)")
            return LBTextRange(c: range)
        }
    }
    
    public func position(from position: UITextPosition, offset: Int) -> UITextPosition? {
        let start = (position as! LBTextPos).c
        let new = position_offset(editorHandle, start, Int32(offset))
        if new.none {
            //            print("\(#function)(\(start), \(offset)) -> nil")
            return nil
        }
        //        print("\(#function)(\(start), \(offset)) -> \(new)")
        return LBTextPos(c: new)
    }
    
    public func position(from position: UITextPosition, in direction: UITextLayoutDirection, offset: Int) -> UITextPosition? {
        let start = (position as! LBTextPos).c
        let direction = CTextLayoutDirection(rawValue: UInt32(direction.rawValue));
        let new = position_offset_in_direction(editorHandle, start, direction, Int32(offset))
        if new.none {
            print("\(#function)(\(start), \(offset)) -> nil")
            return nil
        }
        print("\(#function)(\(start), \(offset)) -> \(new)")
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
        //        print("\(#function)(\(left), \(right)) -> \(res)")
        return res
    }
    
    public func offset(from: UITextPosition, to toPosition: UITextPosition) -> Int {
        let left = Int((from as! LBTextPos).c.pos)
        let right = Int((toPosition as! LBTextPos).c.pos)
        let res = abs(right - left)
        //        print("\(#function)(\(left), \(right)) -> \(res)")
        return res
    }
    
    public var inputDelegate: UITextInputDelegate?
    
    public lazy var tokenizer: UITextInputTokenizer = LBTokenizer(editorHandle: self.editorHandle)
    
    public func position(within range: UITextRange, farthestIn direction: UITextLayoutDirection) -> UITextPosition? {
        print("\(#function)")
        unimplemented()
        return nil
    }
    
    public func characterRange(byExtending position: UITextPosition, in direction: UITextLayoutDirection) -> UITextRange? {
        print("\(#function)")
        unimplemented()
        return nil
    }
    
    public func baseWritingDirection(for position: UITextPosition, in direction: UITextStorageDirection) -> NSWritingDirection {
        //        print("\(#function)")
        //        unimplemented()
        return NSWritingDirection.leftToRight
    }
    
    public func setBaseWritingDirection(_ writingDirection: NSWritingDirection, for range: UITextRange) {
        if writingDirection != .leftToRight {
            unimplemented()
        }
        //        print("\(#function)")
    }
    
    public func firstRect(for range: UITextRange) -> CGRect {
        let range = (range as! LBTextRange).c
        let result = first_rect(editorHandle, range)
        return CGRect(x: result.min_x, y: result.min_y, width: result.max_x-result.min_x, height: result.max_y-result.min_y)
    }
    
    public func caretRect(for position: UITextPosition) -> CGRect {
        let position = (position as! LBTextPos).c
        let result = cursor_rect_at_position(editorHandle, position)
        return CGRect(x: result.min_x, y: result.min_y, width: result.max_x-result.min_x, height: result.max_y-result.min_y)
    }
    
    public func selectionRects(for range: UITextRange) -> [UITextSelectionRect] {
        print("\(#function)")
        unimplemented()
        return []
    }
    
    public func closestPosition(to point: CGPoint) -> UITextPosition? {
        let point = CPoint(x: point.x, y: point.y)
        let result = position_at_point(editorHandle, point)
        return LBTextPos(c: result)
    }
    
    public func closestPosition(to point: CGPoint, within range: UITextRange) -> UITextPosition? {
        print("\(#function)")
        unimplemented()
        return nil
    }
    
    public func characterRange(at point: CGPoint) -> UITextRange? {
        print("\(#function)")
        unimplemented()
        return nil
    }
    
    public var hasText: Bool {
        let res = has_text(editorHandle)
        //        print("\(#function) -> \(res)")
        return res
    }
    
    public func deleteBackward() {
        print("\(#function)")
        backspace(editorHandle)
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
    
    public func editMenuInteraction(_ interaction: UIEditMenuInteraction, menuFor configuration: UIEditMenuConfiguration, suggestedActions: [UIMenuElement]) -> UIMenu? {
        
        var actions = suggestedActions
        
        let customMenu = self.hasSelection ? UIMenu(title: "", options: .displayInline, children: [
            UIAction(title: "Cut") { _ in
                self.clipboardCut()
            },
            UIAction(title: "Copy") { _ in
                self.clipboardCopy()
            },
            UIAction(title: "Paste") { _ in
                self.clipboardPaste()
            },
            UIAction(title: "Select All") { _ in
                select_all(self.editorHandle)
                self.setNeedsDisplay(self.frame)
            },
        ]) : UIMenu(title: "", options: .displayInline, children: [
            UIAction(title: "Select") { _ in
                select_current_word(self.editorHandle)
                self.setNeedsDisplay(self.frame)
            },
            UIAction(title: "Select All") { _ in
                select_all(self.editorHandle)
                self.setNeedsDisplay(self.frame)
            },
            UIAction(title: "Paste") { _ in
                self.clipboardPaste()
            },
        ])
        
        actions.append(customMenu)
        
        return UIMenu(children: customMenu.children)
    }
    
    @objc func clipboardCopy() {
        clipboard_copy(self.editorHandle)
        self.setNeedsDisplay(self.frame)
    }
    
    @objc func clipboardCut() {
        clipboard_cut(self.editorHandle)
        self.setNeedsDisplay(self.frame)
    }
    
    @objc func clipboardPaste() {
        self.setClipboard()
        clipboard_paste(self.editorHandle)
        self.setNeedsDisplay()
    }
    
    @objc func keyboardSelectAll() {
        select_all(self.editorHandle)
        self.setNeedsDisplay()
    }
    
    func updateText(_ s: String) {
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
        return [
            UIKeyCommand(input: "c", modifierFlags: .command, action: #selector(clipboardCopy)),
            UIKeyCommand(input: "x", modifierFlags: .command, action: #selector(clipboardCut)),
            UIKeyCommand(input: "v", modifierFlags: .command, action: #selector(clipboardPaste)),
            UIKeyCommand(input: "a", modifierFlags: .command, action: #selector(keyboardSelectAll)),
        ]
    }
    
    deinit {
        withUnsafePointer(to: self) { pointer in
            print("editor deinited: \(self.editorHandle)")
        }
        deinit_editor(editorHandle)
    }
    
    required init(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }
    
    func unimplemented() {
        print("unimplemented!")
        Thread.callStackSymbols.forEach{print($0)}
        exit(-69)
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
#endif
