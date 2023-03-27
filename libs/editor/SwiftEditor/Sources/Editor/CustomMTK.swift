import MetalKit
import Bridge

public class CustomMTK: MTKView  {
    
    var trackingArea : NSTrackingArea?
    
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
    
    override init(frame frameRect: CGRect, device: MTLDevice?) {
        super.init(frame: frameRect, device: device)
        self.isPaused = true
        self.enableSetNeedsDisplay = true
    }
    
    required init(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }
    
    public override var acceptsFirstResponder: Bool {
        return true
    }
    
    public override func mouseDragged(with event: NSEvent) {
        let local = viewCoordinates(event)
        mouse_moved(editor(), Float(local.x), Float(local.y))
        setNeedsDisplay(self.frame)
    }
    
    public override func mouseMoved(with event: NSEvent) {
        let local = viewCoordinates(event)
        mouse_moved(editor(), Float(local.x), Float(local.y))
        setNeedsDisplay(self.frame)
    }
    
    public override func mouseDown(with event: NSEvent) {
        let local = viewCoordinates(event)
        mouse_button(editor(), Float(local.x), Float(local.y), true, true)
        setNeedsDisplay(self.frame)
    }
    
    public override func mouseUp(with event: NSEvent) {
        let local = viewCoordinates(event)
        mouse_button(editor(), Float(local.x), Float(local.y), false, true)
        delegate().maybeDirty()
        setNeedsDisplay(self.frame)
    }
    
    public override func rightMouseDown(with event: NSEvent) {
        let local = viewCoordinates(event)
        mouse_button(editor(), Float(local.x), Float(local.y), true, false)
        setNeedsDisplay(self.frame)
    }
    
    public override func rightMouseUp(with event: NSEvent) {
        let local = viewCoordinates(event)
        mouse_button(editor(), Float(local.x), Float(local.y), false, false)
        setNeedsDisplay(self.frame)
    }
    
    
    public override func scrollWheel(with event: NSEvent) {
        scroll_wheel(editor(), Float(event.scrollingDeltaY))
        setNeedsDisplay(self.frame)
    }
    
    public override func keyDown(with event: NSEvent) {
        print("down \(event.keyCode), \(event.modifierFlags), \(event.characters)")
        key_event(editor(), event.keyCode, event.modifierFlags.contains(.shift), event.modifierFlags.contains(.control), event.modifierFlags.contains(.option), event.modifierFlags.contains(.command), true, event.characters)
        setNeedsDisplay(self.frame)
    }
    
    public override func viewDidChangeEffectiveAppearance() {
        dark_mode(editor(), isDarkMode())
        setNeedsDisplay(self.frame)
    }
    
    // https://stackoverflow.com/a/53218688/1060955
    func isDarkMode() -> Bool {
        return self.effectiveAppearance.bestMatch(from: [.darkAqua, .aqua]) == .darkAqua
    }
    
    public override func keyUp(with event: NSEvent) {
        key_event(editor(), event.keyCode, event.modifierFlags.contains(.shift), event.modifierFlags.contains(.control), event.modifierFlags.contains(.option), event.modifierFlags.contains(.command), false, event.characters)
        delegate().maybeDirty()
        setNeedsDisplay(self.frame)
    }
    
    func delegate() -> FrameManager {
        (self.delegate as! FrameManager)
    }
    
    func editor() -> UnsafeMutableRawPointer {
        delegate().editorHandle
    }
    
    func viewCoordinates(_ event: NSEvent) -> NSPoint {
        var local = self.convert(event.locationInWindow, from: nil)
        local.y = self.frame.size.height - local.y
        return local
    }
}

public protocol TextLoader {
    func textReloadNeeded() -> Bool
    func textReloaded()
    func loadText() -> String
    func documentChanged(s: String)
}

public class FrameManager: NSObject, MTKViewDelegate {
    var editorHandle: UnsafeMutableRawPointer
    var loader: TextLoader
    var parent: CustomMTK
    var timer: Timer?
    var pasteBoardEventId: Int = 0
    
    public init(_ parent: CustomMTK, _ loader: TextLoader) {
        self.parent = parent
        self.loader = loader
        let metalLayer = UnsafeMutableRawPointer(Unmanaged.passRetained(self.parent.layer!).toOpaque())
        self.editorHandle = init_editor(metalLayer, self.loader.loadText(), parent.isDarkMode())
        super.init()
        DispatchQueue.main.async {
            async {
                try await self.checkForChanges()
            }
        }
    }
    
    func checkForChanges() async throws {
        while true {
            if self.loader.textReloadNeeded() {                
                self.reloadText()
            }
            try await Task.sleep(nanoseconds: 500000000)
        }
    }
    
    deinit {
        deinit_editor(editorHandle)
    }
    
    public func mtkView(_ view: MTKView, drawableSizeWillChange size: CGSize) {
        let scale = self.parent.window?.backingScaleFactor ?? 1.0
        print(Float(size.width), Float(size.height), Float(scale))
        resize_editor(editorHandle, Float(size.width), Float(size.height), Float(scale))
    }
    
    func reloadText() {
        let text = self.loader.loadText()
        set_text(editorHandle, text)
        self.parent.setNeedsDisplay(self.parent.frame)
        print("called new text \(text)")
        self.loader.textReloaded()
    }
    
    public func draw(in view: MTKView) {
        if NSPasteboard.general.changeCount != self.pasteBoardEventId {
            setClipboard()
        }
        
        let scale = Float(self.parent.window?.backingScaleFactor ?? 1.0)
        dark_mode(editorHandle, (view as! CustomMTK).isDarkMode())
        set_scale(editorHandle, scale)
        draw_editor(editorHandle)
        if has_coppied_text(editorHandle) {
            NSPasteboard.general.clearContents()
            NSPasteboard.general.setString(getCoppiedText(), forType: .string)
        }
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
        let result = get_coppied_text(editorHandle)
        let str = String(cString: result!)
        free_text(UnsafeMutablePointer(mutating: result))
        return str
    }
    
    func maybeDirty() {
        let string = self.getText()
        self.loader.documentChanged(s: string)
    }
    
    func getText() -> String {
        let result = get_text(editorHandle)
        let str = String(cString: result!)
        free_text(UnsafeMutablePointer(mutating: result))
        return str
    }
}
