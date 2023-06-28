#if os(macOS)
import MetalKit
import Bridge

public class MacMTK: MTKView, MTKViewDelegate {
    
    var editorHandle: UnsafeMutableRawPointer?
    var trackingArea : NSTrackingArea?
    var pasteBoardEventId: Int = 0
    
    var editorState: EditorState?
    
    override init(frame frameRect: CGRect, device: MTLDevice?) {
        super.init(frame: frameRect, device: device)
        self.delegate = self
        self.isPaused = true
        self.enableSetNeedsDisplay = true
    }
    
    public override func updateTrackingAreas() {
        if trackingArea != nil {
            self.removeTrackingArea(trackingArea!)
        }
        let options : NSTrackingArea.Options =
        [.mouseEnteredAndExited, .mouseMoved, .enabledDuringMouseDrag, .activeInKeyWindow]
        trackingArea = NSTrackingArea(rect: self.bounds, options: options,
                                      owner: self, userInfo: nil)
        self.addTrackingArea(trackingArea!)
    }

    required init(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }
    
    public func header(headingSize: UInt32) {
        apply_style_to_selection_header(editorHandle, headingSize)
        setNeedsDisplay(self.frame)
    }

    public func bulletedList() {
        apply_style_to_selection_bulleted_list(editorHandle)
        setNeedsDisplay(self.frame)
    }

    public func numberedList() {
        apply_style_to_selection_numbered_list(editorHandle)
        setNeedsDisplay(self.frame)
    }

    public func todoList() {
        apply_style_to_selection_todo_list(editorHandle)
        setNeedsDisplay(self.frame)
    }

    public func bold() {
        apply_style_to_selection_bold(editorHandle)
        setNeedsDisplay(self.frame)
    }

    public func italic() {
        apply_style_to_selection_italic(editorHandle)
        setNeedsDisplay(self.frame)
    }

    public func inlineCode() {
        apply_style_to_selection_inline_code(editorHandle)
        self.setNeedsDisplay(self.frame)
    }
    
    public func automaticTitleComputation(_ computeTitle: Bool) {
        print("SETTING THE STUFF \(editorHandle)")
        set_automatic_title_computation(editorHandle, computeTitle)
    }

    public override var acceptsFirstResponder: Bool {
        return true
    }
    
    public func setInitialContent(_ s: String) {
        let metalLayer = UnsafeMutableRawPointer(Unmanaged.passUnretained(self.layer!).toOpaque())
        self.editorHandle = init_editor(metalLayer, s, isDarkMode())
        print("THE STUFF \(editorHandle)")
    }
    
    public override func mouseDragged(with event: NSEvent) {
        let local = viewCoordinates(event)
        mouse_moved(editorHandle, Float(local.x), Float(local.y))
        setNeedsDisplay(self.frame)
    }
    
    public override func mouseMoved(with event: NSEvent) {
        let local = viewCoordinates(event)
        mouse_moved(editorHandle, Float(local.x), Float(local.y))
        setNeedsDisplay(self.frame)
    }
    
    public override func mouseDown(with event: NSEvent) {
        let local = viewCoordinates(event)
        mouse_button(editorHandle, Float(local.x), Float(local.y), true, true, event.modifierFlags.contains(.shift), event.modifierFlags.contains(.control), event.modifierFlags.contains(.option), event.modifierFlags.contains(.command))
        setNeedsDisplay(self.frame)
    }
    
    public override func mouseUp(with event: NSEvent) {
        let local = viewCoordinates(event)
        mouse_button(editorHandle, Float(local.x), Float(local.y), false, true, event.modifierFlags.contains(.shift), event.modifierFlags.contains(.control), event.modifierFlags.contains(.option), event.modifierFlags.contains(.command))
        self.textChanged()
        setNeedsDisplay(self.frame)
    }
    
    public override func rightMouseDown(with event: NSEvent) {
        let local = viewCoordinates(event)
        mouse_button(editorHandle, Float(local.x), Float(local.y), true, false, event.modifierFlags.contains(.shift), event.modifierFlags.contains(.control), event.modifierFlags.contains(.option), event.modifierFlags.contains(.command))
        setNeedsDisplay(self.frame)
    }
    
    public override func rightMouseUp(with event: NSEvent) {
        let local = viewCoordinates(event)
        mouse_button(editorHandle, Float(local.x), Float(local.y), false, false, event.modifierFlags.contains(.shift), event.modifierFlags.contains(.control), event.modifierFlags.contains(.option), event.modifierFlags.contains(.command))
        setNeedsDisplay(self.frame)
    }
    
    public override func scrollWheel(with event: NSEvent) {
        scroll_wheel(editorHandle, Float(event.scrollingDeltaY))
        setNeedsDisplay(self.frame)
    }
    
    public override func keyDown(with event: NSEvent) {
        setClipboard()
        key_event(editorHandle, event.keyCode, event.modifierFlags.contains(.shift), event.modifierFlags.contains(.control), event.modifierFlags.contains(.option), event.modifierFlags.contains(.command), true, event.characters)
        setNeedsDisplay(self.frame)
    }
    
    public override func viewDidChangeEffectiveAppearance() {
        dark_mode(editorHandle, isDarkMode())
        setNeedsDisplay(self.frame)
    }
    
    // https://stackoverflow.com/a/53218688/1060955
    func isDarkMode() -> Bool {
        self.effectiveAppearance.bestMatch(from: [.darkAqua, .aqua]) == .darkAqua
    }
    
    public override func keyUp(with event: NSEvent) {
        key_event(editorHandle, event.keyCode, event.modifierFlags.contains(.shift), event.modifierFlags.contains(.control), event.modifierFlags.contains(.option), event.modifierFlags.contains(.command), false, event.characters)
        self.textChanged()
        setNeedsDisplay(self.frame)
    }
    
    func setClipboard(){
        let pasteboardString: String? = NSPasteboard.general.string(forType: .string)
        if let theString = pasteboardString {
            print("clipboard contents: \(theString)")
            system_clipboard_changed(editorHandle, theString)
        }
        self.pasteBoardEventId = NSPasteboard.general.changeCount
    }
    
    func getCoppiedText() -> String {
        let result = get_copied_text(editorHandle)
        let str = String(cString: result!)
        free_text(UnsafeMutablePointer(mutating: result))
        return str
    }
    
    func viewCoordinates(_ event: NSEvent) -> NSPoint {
        var local = self.convert(event.locationInWindow, from: nil)
        local.y = self.frame.size.height - local.y
        return local
    }
    
    public func mtkView(_ view: MTKView, drawableSizeWillChange size: CGSize) {
        // initially window is not set, this defaults to 1.0, initial frame comes from `init_editor`
        // we probably want a setNeedsDisplay here
        let scale = self.window?.backingScaleFactor ?? 1.0
        print(Float(size.width), Float(size.height), Float(scale))
        resize_editor(editorHandle, Float(size.width), Float(size.height), Float(scale))
    }
    
    public func draw(in view: MTKView) {
        if NSPasteboard.general.changeCount != self.pasteBoardEventId {
            setClipboard()
        }
        
        let scale = Float(self.window?.backingScaleFactor ?? 1.0)
        dark_mode(editorHandle, isDarkMode())
        set_scale(editorHandle, scale)
        let output = draw_editor(editorHandle)

        editorState?.isHeadingSelected = output.editor_response.cursor_in_heading;
        editorState?.isTodoListSelected = output.editor_response.cursor_in_todo_list;
        editorState?.isBulletListSelected = output.editor_response.cursor_in_bullet_list;
        editorState?.isNumberListSelected = output.editor_response.cursor_in_number_list;
        editorState?.isInlineCodeSelected = output.editor_response.cursor_in_inline_code;
        editorState?.isBoldSelected = output.editor_response.cursor_in_bold;
        editorState?.isItalicSelected = output.editor_response.cursor_in_italic;

        if let potentialTitle = output.editor_response.potential_title {
            editorState?.potentialTitle = String(cString: potentialTitle)
        } else {
            editorState?.potentialTitle = nil
        }
        
        view.isPaused = !output.redraw
        if has_copied_text(editorHandle) {
            NSPasteboard.general.clearContents()
            NSPasteboard.general.setString(getCoppiedText(), forType: .string)
        }
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
    
    deinit {
         deinit_editor(editorHandle)
    }
}
#endif
