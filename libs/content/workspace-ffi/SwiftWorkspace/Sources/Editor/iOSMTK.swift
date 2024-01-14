#if os(iOS)
import UIKit
import MetalKit
import Bridge
import SwiftUI
import MobileCoreServices
import UniformTypeIdentifiers

public class iOSMTK: MTKView, MTKViewDelegate, UITextInput, UIEditMenuInteractionDelegate, UIDropInteractionDelegate {
    
    var wsHandle: UnsafeMutableRawPointer?
    var workspaceState: WorkspaceState?
    var hasSelection: Bool = false
    var currentOpenDoc: UUID? = nil
    var currentSelectedFolder: UUID? = nil
    
    var textUndoManager = iOSUndoManager()

    var redrawTask: DispatchWorkItem? = nil
    
    let textInteraction = UITextInteraction(for: .editable)
    
    var inTextTab = false

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
        self.clipsToBounds = true

        // ipad trackpad support
        let pan = UIPanGestureRecognizer(target: self, action: #selector(self.handleTrackpadScroll(_:)))
        pan.allowedScrollTypesMask = .all
        pan.maximumNumberOfTouches  = 0
        self.addGestureRecognizer(pan)
        
        // selection support
        textInteraction.textInput = self
        
        for gestureRecognizer in textInteraction.gesturesForFailureRequirements {
            let gestureName = gestureRecognizer.name?.lowercased()

            if(gestureName?.contains("tap") ?? false) {
                gestureRecognizer.cancelsTouchesInView = false
            }
        }

        // drop support
        let dropInteraction = UIDropInteraction(delegate: self)
        self.addInteraction(dropInteraction)
    }
    
    func openFile(id: UUID) {
        if currentOpenDoc != id {
            let uuid = CUuid(_0: id.uuid)
            open_file(wsHandle, uuid, false)
            setNeedsDisplay(self.frame)
        }

    }
    
    func requestSync() {
        request_sync(wsHandle)
        setNeedsDisplay(self.frame)
    }
    
    public func dropInteraction(_ interaction: UIDropInteraction, canHandle session: UIDropSession) -> Bool {
        if !inTextTab {
            return false
        }
        
        guard session.items.count == 1 else { return false }
        
        return session.hasItemsConforming(toTypeIdentifiers: [UTType.image.identifier, UTType.fileURL.identifier, UTType.text.identifier])
    }

    public func dropInteraction(_ interaction: UIDropInteraction, sessionDidUpdate session: UIDropSession) -> UIDropProposal {
        let dropLocation = session.location(in: self)
        let operation: UIDropOperation

        if inTextTab && self.frame.contains(dropLocation) {
            operation = .copy
        } else {
            operation = .cancel
        }

        return UIDropProposal(operation: operation)
    }

    public func dropInteraction(_ interaction: UIDropInteraction, performDrop session: UIDropSession) {
        if !inTextTab {
            return
        }
        
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
            scroll_wheel(wsHandle, y - lastKnownTapLocation!)
            
            lastKnownTapLocation = y
            self.setNeedsDisplay()
        }
        
    }

    func importContent(_ importFormat: SupportedImportFormat) -> Bool {
        switch importFormat {
        case .url(let url):
            if let markdownURL = workspaceState!.importFile(url) {
                paste_text(wsHandle, markdownURL)
                workspaceState?.pasted = true
                
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

                if let lbImageURL = workspaceState!.importFile(imageUrl) {
                    paste_text(wsHandle, lbImageURL)
                    workspaceState?.pasted = true

                    return true
                }
            }
        case .text(let text):
            paste_text(wsHandle, text)
            workspaceState?.pasted = true

            return true
        }
        
        return false
    }

    public func setInitialContent(_ coreHandle: UnsafeMutableRawPointer?) {
        let metalLayer = UnsafeMutableRawPointer(Unmanaged.passUnretained(self.layer).toOpaque())
        self.wsHandle = init_ws(coreHandle, metalLayer, isDarkMode())
        self.textUndoManager.wsHandle = self.wsHandle
        self.textUndoManager.onUndoRedo = {
            self.setNeedsDisplay(self.frame)
        }
    }
    
    public func mtkView(_ view: MTKView, drawableSizeWillChange size: CGSize) {
        resize_editor(wsHandle, Float(size.width), Float(size.height), Float(self.contentScaleFactor))
        self.setNeedsDisplay()
    }
    
    public func draw(in view: MTKView) {
        dark_mode(wsHandle, isDarkMode())
        set_scale(wsHandle, Float(self.contentScaleFactor))
        let output = draw_editor(wsHandle)
//
//        if output.editor_response.selection_updated || output.editor_response.scroll_updated {
//            inputDelegate?.selectionDidChange(self)
//        }
//
//        if output.editor_response.text_updated {
//            inputDelegate?.textDidChange(self)
//        }
        
        workspaceState?.statusMsg = textFromPtr(s: output.workspace_resp.msg)
        workspaceState?.syncing = output.workspace_resp.syncing
        workspaceState?.reloadFiles = output.workspace_resp.refresh_files
        
        let selectedFile = UUID(uuid: output.workspace_resp.selected_file._0)
        
        if selectedFile.isNil() {
            currentOpenDoc = nil
            if self.workspaceState?.openDoc != nil {
                inTextTab = false
                self.workspaceState?.openDoc = nil
            }
        } else {
            if currentOpenDoc != selectedFile {
                let newInTextTab = in_text_tab(wsHandle)
                if newInTextTab && !self.inTextTab {
                    self.addInteraction(textInteraction)
                    self.inTextTab = true
                } else if !newInTextTab && self.inTextTab {
                    self.removeInteraction(textInteraction)
                    self.inTextTab = false
                }
                
                inTextTab = newInTextTab
            }
            
            currentOpenDoc = selectedFile
            if selectedFile != self.workspaceState?.openDoc {
                self.workspaceState?.openDoc = selectedFile
            }
        }
                
        let newFile = UUID(uuid: output.workspace_resp.doc_created._0)
        if !newFile.isNil() {
            self.workspaceState?.openDoc = newFile
        }
        
        if output.workspace_resp.new_folder_btn_pressed {
            workspaceState?.newFolderButtonPressed = true
        }
        
        if let openedUrl = output.url_opened {
            let url = textFromPtr(s: openedUrl)
            
            if let url = URL(string: url),
                UIApplication.shared.canOpenURL(url) {
                UIApplication.shared.open(url)
            }
        }
        
        if let text = output.copied_text {
            UIPasteboard.general.string = textFromPtr(s: text)
        }

        redrawTask?.cancel()
        self.isPaused = output.redraw_in > 100
        if self.isPaused {
            let redrawIn = Int(truncatingIfNeeded: output.redraw_in)
            
            if redrawIn != -1 {
                let newRedrawTask = DispatchWorkItem {
                    self.setNeedsDisplay(self.frame)
                }
                DispatchQueue.main.asyncAfter(deadline: .now() + .milliseconds(redrawIn), execute: newRedrawTask)
                redrawTask = newRedrawTask
            }
        }
    }
    
    func setClipboard() {
        let pasteboardString: String? = UIPasteboard.general.string
        if let theString = pasteboardString {
            system_clipboard_changed(wsHandle, theString)
        }
        self.pasteBoardEventId = UIPasteboard.general.changeCount
    }
    
    public func insertText(_ text: String) {
        if !inTextTab {
            return
        }
        
        inputDelegate?.textWillChange(self)
        insert_text(wsHandle, text)
        self.setNeedsDisplay(self.frame)
    }
    
    public func text(in range: UITextRange) -> String? {
        if !inTextTab {
            return nil
        }
        
        let range = (range as! LBTextRange).c
        guard let result = text_in_range(wsHandle, range) else {
            return nil
        }
        let str = String(cString: result)
        free_text(UnsafeMutablePointer(mutating: result))
        return str
    }
    
    
    public func replace(_ range: UITextRange, withText text: String) {
        if !inTextTab {
            return
        }
        
        let range = range as! LBTextRange
        inputDelegate?.textWillChange(self)
        replace_text(wsHandle, range.c, text)
        self.setNeedsDisplay(self.frame)
    }
    
    public var selectedTextRange: UITextRange? {
        set {
            if !inTextTab {
                return
            }
            
            guard let range = (newValue as? LBTextRange)?.c else {
                return
            }
            inputDelegate?.selectionWillChange(self)
            set_selected(wsHandle, range)
            self.setNeedsDisplay()
        }
        
        get {
            if !inTextTab {
                return nil
            }
            
            let range = get_selected(wsHandle)
            if range.none {
                return nil
            }
            return LBTextRange(c: range)
        }
    }
    
    public var markedTextRange: UITextRange? {
        get {
            if !inTextTab {
                return nil
            }
            
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
        if !inTextTab {
            return
        }
        
        inputDelegate?.textWillChange(self)
        set_marked(wsHandle, CTextRange(none: false, start: CTextPosition(none: false, pos: UInt(selectedRange.lowerBound)), end: CTextPosition(none: false, pos: UInt(selectedRange.upperBound))), markedText)
        self.setNeedsDisplay()
    }
    
    public func unmarkText() {
        if !inTextTab {
            return
        }
        
        inputDelegate?.textWillChange(self)
        unmark_text(wsHandle)
        self.setNeedsDisplay()
    }
    
    public var beginningOfDocument: UITextPosition {
        if !inTextTab {
            return LBTextPos(c: CTextPosition(none: true, pos: 0))
        }
        
        let res = beginning_of_document(wsHandle)
        return LBTextPos(c: res)
    }
    
    public var endOfDocument: UITextPosition {
        if !inTextTab {
            return LBTextPos(c: CTextPosition(none: true, pos: 0))
        }
        
        let res = end_of_document(wsHandle)
        return LBTextPos(c: res)
    }
    
    public func textRange(from fromPosition: UITextPosition, to toPosition: UITextPosition) -> UITextRange? {
        if !inTextTab {
            return nil
        }
        
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
        if !inTextTab {
            return nil
        }
        
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
        if !inTextTab {
            return nil
        }
        
        let start = (position as! LBTextPos).c
        let direction = CTextLayoutDirection(rawValue: UInt32(direction.rawValue));
        let new = position_offset_in_direction(wsHandle, start, direction, Int32(offset))
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
        if !inTextTab {
            return CGRect(x: 0, y: 0, width: 0, height: 0)
        }
        
        let range = (range as! LBTextRange).c
        let result = first_rect(wsHandle, range)
        return CGRect(x: result.min_x, y: result.min_y, width: result.max_x-result.min_x, height: result.max_y-result.min_y)
    }
    
    public func caretRect(for position: UITextPosition) -> CGRect {
        if !inTextTab {
            return CGRect(x: 0, y: 0, width: 0, height: 0)
        }
        
        let position = (position as! LBTextPos).c
        let result = cursor_rect_at_position(wsHandle, position)
        return CGRect(x: result.min_x, y: result.min_y, width: 1, height:result.max_y - result.min_y)
    }
    
    public func selectionRects(for range: UITextRange) -> [UITextSelectionRect] {
        if !inTextTab {
            return []
        }
        
        let range = (range as! LBTextRange).c
        let result = selection_rects(wsHandle, range)
        let buffer = Array(UnsafeBufferPointer(start: result.rects, count: Int(result.size)))
        return buffer.enumerated().map { (index, rect) in
            return LBTextSelectionRect(cRect: rect, loc: index, size: buffer.count)
        }
    }
    
    public func closestPosition(to point: CGPoint) -> UITextPosition? {
        if !inTextTab || point.y < 50 {
            return nil
        }
        
        let point = CPoint(x: point.x, y: point.y)
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
        if !inTextTab {
            return
        }
        
        inputDelegate?.textWillChange(self)
        backspace(wsHandle)
        self.setNeedsDisplay(self.frame)
    }
    
    public override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent?) {
        let point = Unmanaged.passUnretained(touches.first!).toOpaque()
        let value = UInt64(UInt(bitPattern: point))
        let location = touches.first!.location(in: self)
        touches_began(wsHandle, value, Float(location.x), Float(location.y), Float(touches.first?.force ?? 0))

        self.setNeedsDisplay(self.frame)
    }
    
    public override func touchesMoved(_ touches: Set<UITouch>, with event: UIEvent?) {
        let point = Unmanaged.passUnretained(touches.first!).toOpaque()
        let value = UInt64(UInt(bitPattern: point))
        let location = touches.first!.location(in: self)
        touches_moved(wsHandle, value, Float(location.x), Float(location.y), Float(touches.first?.force ?? 0))
        self.setNeedsDisplay(self.frame)
    }
    
    public override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent?) {
        let point = Unmanaged.passUnretained(touches.first!).toOpaque()
        let value = UInt64(UInt(bitPattern: point))
        let location = touches.first!.location(in: self)
        touches_ended(wsHandle, value, Float(location.x), Float(location.y), Float(touches.first?.force ?? 0))
        self.setNeedsDisplay(self.frame)
    }
    
    
    public override func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent?) {
        let point = Unmanaged.passUnretained(touches.first!).toOpaque()
        let value = UInt64(UInt(bitPattern: point))
        let location = touches.first!.location(in: self)
        touches_cancelled(wsHandle, value, Float(location.x), Float(location.y), Float(touches.first?.force ?? 0))
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
                select_all(self.wsHandle)
                self.setNeedsDisplay(self.frame)
            },
        ]) : UIMenu(title: "", options: .displayInline, children: [
            UIAction(title: "Select") { _ in
                self.inputDelegate?.selectionWillChange(self)
                select_current_word(self.wsHandle)
                self.setNeedsDisplay(self.frame)
            },
            UIAction(title: "Select All") { _ in
                self.inputDelegate?.selectionWillChange(self)
                select_all(self.wsHandle)
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
        clipboard_copy(self.wsHandle)
        self.setNeedsDisplay(self.frame)
    }
    
    @objc func clipboardCut() {
        inputDelegate?.textWillChange(self)
        clipboard_cut(self.wsHandle)
        self.setNeedsDisplay(self.frame)
    }
    
    @objc func clipboardPaste() {
        self.setClipboard()
        
        if let image = UIPasteboard.general.image {
            if importContent(.image(image)) {
                return
            }
        }

        clipboard_paste(self.wsHandle)
        self.setNeedsDisplay()
    }
    
    @objc func keyboardSelectAll() {
        inputDelegate?.selectionWillChange(self)
        select_all(self.wsHandle)
        self.setNeedsDisplay()
    }
        
    func undoRedo(redo: Bool) {
        inputDelegate?.textWillChange(self)
        undo_redo(self.wsHandle, redo)
        self.setNeedsDisplay(self.frame)
    }
    
    func getText() -> String {
        if !inTextTab {
            return ""
        }
        
        let result = get_text(wsHandle)
        let str = String(cString: result!)
        free_text(UnsafeMutablePointer(mutating: result))
        return str
    }
    
    public override var canBecomeFirstResponder: Bool {
        return true
    }
    
    override public func traitCollectionDidChange(_ previousTraitCollection: UITraitCollection?) {
        dark_mode(wsHandle, isDarkMode())
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
        deinit_editor(wsHandle)
    }
    
    @objc func deleteWord() {
        if !inTextTab {
            return
        }
        
        delete_word(wsHandle)
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
    let wsHandle: UnsafeMutableRawPointer?
    
    init(wsHandle: UnsafeMutableRawPointer?) {
        self.wsHandle = wsHandle
    }
    
    func isPosition(_ position: UITextPosition, atBoundary granularity: UITextGranularity, inDirection direction: UITextDirection) -> Bool {
        guard let position = (position as? LBTextPos)?.c else {
            return false
        }
        let granularity = CTextGranularity(rawValue: UInt32(granularity.rawValue))
        let backwards = direction.rawValue == 1
        return is_position_at_bound(wsHandle, position, granularity, backwards)
    }
    
    func isPosition(_ position: UITextPosition, withinTextUnit granularity: UITextGranularity, inDirection direction: UITextDirection) -> Bool {
        guard let position = (position as? LBTextPos)?.c else {
            return false
        }
        let granularity = CTextGranularity(rawValue: UInt32(granularity.rawValue))
        let backwards = direction.rawValue == 1
        return is_position_within_bound(wsHandle, position, granularity, backwards)
    }
    
    func position(from position: UITextPosition, toBoundary granularity: UITextGranularity, inDirection direction: UITextDirection) -> UITextPosition? {
        guard let position = (position as? LBTextPos)?.c else {
            return nil
        }
        let granularity = CTextGranularity(rawValue: UInt32(granularity.rawValue))
        let backwards = direction.rawValue == 1
        let result = bound_from_position(wsHandle, position, granularity, backwards)
        return LBTextPos(c: result)
    }
    
    func rangeEnclosingPosition(_ position: UITextPosition, with granularity: UITextGranularity, inDirection direction: UITextDirection) -> UITextRange? {
        guard let position = (position as? LBTextPos)?.c else {
            return nil
        }
        let granularity = CTextGranularity(rawValue: UInt32(granularity.rawValue))
        let backwards = direction.rawValue == 1
        let result = bound_at_position(wsHandle, position, granularity, backwards)
        return LBTextRange(c: result)
    }
}

class iOSUndoManager: UndoManager {
    
    public var wsHandle: UnsafeMutableRawPointer? = nil
    var onUndoRedo: (() -> Void)? = nil
    
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
        undo_redo(wsHandle, false)
        onUndoRedo?()
    }
    
    override func redo() {
        undo_redo(wsHandle, true)
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
