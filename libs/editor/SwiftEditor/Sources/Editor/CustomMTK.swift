import MetalKit
import Bridge

class CustomMTK: MTKView  {
    
    var trackingArea : NSTrackingArea?
    
    override func updateTrackingAreas() {
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
    }
    
    required init(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }
    
    override var acceptsFirstResponder: Bool {
        return true
    }
    
    override func mouseDragged(with event: NSEvent) {
        let local = viewCoordinates(event)
        mouse_moved(editor(), Float(local.x), Float(local.y))
    }
    
    override func mouseMoved(with event: NSEvent) {
        let local = viewCoordinates(event)
        mouse_moved(editor(), Float(local.x), Float(local.y))
    }
    
    override func mouseDown(with event: NSEvent) {
        let local = viewCoordinates(event)
        mouse_button(editor(), Float(local.x), Float(local.y), true, true)
    }
    
    override func mouseUp(with event: NSEvent) {
        let local = viewCoordinates(event)
        mouse_button(editor(), Float(local.x), Float(local.y), false, true)
    }
    
    override func rightMouseDown(with event: NSEvent) {
        let local = viewCoordinates(event)
        mouse_button(editor(), Float(local.x), Float(local.y), true, false)
    }
    
    override func rightMouseUp(with event: NSEvent) {
        let local = viewCoordinates(event)
        mouse_button(editor(), Float(local.x), Float(local.y), false, false)
    }
    
    
    override func scrollWheel(with event: NSEvent) {
        scroll_wheel(editor(), Float(event.scrollingDeltaY)) // todo: get x too
    }
    
    override func keyDown(with event: NSEvent) {
        print("down \(event.keyCode), \(event.modifierFlags), \(event.characters)")
        key_event(editor(), event.keyCode, event.modifierFlags.contains(.shift), event.modifierFlags.contains(.control), event.modifierFlags.contains(.option), event.modifierFlags.contains(.command), true, event.characters)
    }
    
    override func keyUp(with event: NSEvent) {
        key_event(editor(), event.keyCode, event.modifierFlags.contains(.shift), event.modifierFlags.contains(.control), event.modifierFlags.contains(.option), event.modifierFlags.contains(.command), false, event.characters)
        delegate().maybeDirty()
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
    func initialText() -> String
    func documentChanged(s: String)
}

class FrameManager: NSObject, MTKViewDelegate {
    var editorHandle: UnsafeMutableRawPointer
    var loader: TextLoader
    var parent: CustomMTK
    var metalDevice: MTLDevice!
    var metalCommandQueue: MTLCommandQueue!
    
    init(_ parent: CustomMTK, _ loader: TextLoader) {
        self.parent = parent
        self.loader = loader
        let metalLayer = UnsafeMutableRawPointer(Unmanaged.passRetained(self.parent.layer!).toOpaque())
        self.editorHandle = init_editor(metalLayer, self.loader.initialText())
        
        super.init()
    }
    
    deinit {
        deinit_editor(editorHandle)
    }
    
    func mtkView(_ view: MTKView, drawableSizeWillChange size: CGSize) {
        let scale = self.parent.window?.backingScaleFactor ?? 1.0
        print(Float(size.width), Float(size.height), Float(scale))
        resize_editor(editorHandle, Float(size.width), Float(size.height), Float(scale))
    }
    
    func draw(in view: MTKView) { // Ask for frame here?
        let scale = Float(self.parent.window?.backingScaleFactor ?? 1.0)
        set_scale(editorHandle, scale)
        draw_editor(editorHandle)
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
