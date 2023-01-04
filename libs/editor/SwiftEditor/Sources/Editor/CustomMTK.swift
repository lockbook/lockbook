import MetalKit
import Bridge

class CustomMTK: MTKView  {
    override init(frame frameRect: CGRect, device: MTLDevice?) {
        super.init(frame: frameRect, device: device)
    }
    
    required init(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }
    
    override var acceptsFirstResponder: Bool {
        return true
    }
    
    override func scrollWheel(with event: NSEvent) {
        print(event.scrollingDeltaY)
    }
    
    override func keyDown(with event: NSEvent) {
        print("down \(event.keyCode), \(event.modifierFlags), \(event.characters)")
        key_event(obj(), event.keyCode, event.modifierFlags.contains(.shift), event.modifierFlags.contains(.control), event.modifierFlags.contains(.option), event.modifierFlags.contains(.command), true, event.characters)
    }
    
    override func keyUp(with event: NSEvent) {
        print("up \(event.keyCode), \(event.modifierFlags), \(event.characters)")
        key_event(obj(), event.keyCode, event.modifierFlags.contains(.shift), event.modifierFlags.contains(.control), event.modifierFlags.contains(.option), event.modifierFlags.contains(.command), false, event.characters)
    }
    
    func obj() -> UnsafeMutableRawPointer {
        (self.delegate as! FrameManager).editorHandle
    }
}

class FrameManager: NSObject, MTKViewDelegate {
    var editorHandle: UnsafeMutableRawPointer
    var parent: CustomMTK
    var metalDevice: MTLDevice!
    var metalCommandQueue: MTLCommandQueue!
    
    init(_ parent: CustomMTK) {
        self.parent = parent
        print(self.parent.window?.backingScaleFactor)
        let metalLayer = UnsafeMutableRawPointer(Unmanaged.passRetained(self.parent.layer!).toOpaque())
        self.editorHandle = init_editor(metalLayer)
        
        super.init()
    }
    
    func mtkView(_ view: MTKView, drawableSizeWillChange size: CGSize) {
        print("size")
//        print( Float(size.width), Float(size.height), Float(view.layer!.contentsScale))
        let scale = self.parent.window?.backingScaleFactor ?? 1.0
        print(Float(size.width), Float(size.height), Float(scale))
        resize_editor(editorHandle, Float(size.width), Float(size.height), Float(scale))
    }
    
    func draw(in view: MTKView) { // Ask for frame here?
        let scale = Float(self.parent.window?.backingScaleFactor ?? 1.0)
        set_scale(editorHandle, scale)
        draw_editor(editorHandle)
    }
}
