#if os(macOS)
import MetalKit
import Bridge

public class MacMTK: MTKView, MTKViewDelegate {
    
    var wsHandle: UnsafeMutableRawPointer?
    var coreHandle: UnsafeMutableRawPointer?
    var trackingArea : NSTrackingArea?
    var pasteBoardEventId: Int = 0
        
    var workspaceState: WorkspaceState?
    
    // todo this will probably just become us hanging on to the last output
    var currentOpenDoc: UUID? = nil
    var currentSelectedFolder: UUID? = nil
    
    var redrawTask: DispatchWorkItem? = nil
    
    var lastCursor: NSCursor = NSCursor.arrow
    
    override init(frame frameRect: CGRect, device: MTLDevice?) {
        super.init(frame: frameRect, device: device)
        self.delegate = self
        self.isPaused = true
        self.enableSetNeedsDisplay = true
        self.preferredFramesPerSecond = 120
    }
    
    func openFile(id: UUID) {
        let uuid = CUuid(_0: id.uuid)
        open_file(wsHandle, uuid, false)
        setNeedsDisplay(self.frame)
    }
    
    func modifiersChanged(event: NSEvent) -> NSEvent {
        modifier_event(self.wsHandle, event.modifierFlags.contains(.shift), event.modifierFlags.contains(.control), event.modifierFlags.contains(.option), event.modifierFlags.contains(.command))
        setNeedsDisplay(self.frame)
        return event
    }
    
    public override func resetCursorRects() {
        addCursorRect(self.frame, cursor: lastCursor)
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

    public override var acceptsFirstResponder: Bool {
        return true
    }
    
    public func setInitialContent(_ coreHandle: UnsafeMutableRawPointer?) {
        print("initial content called")
        let metalLayer = UnsafeMutableRawPointer(Unmanaged.passUnretained(self.layer!).toOpaque())
        self.wsHandle = init_ws(coreHandle, metalLayer, isDarkMode())
        
        NSEvent.addLocalMonitorForEvents(matching: .flagsChanged, handler: modifiersChanged(event:))
        registerForDraggedTypes([.png, .tiff, .fileURL, .string])
        becomeFirstResponder()
    }
    
    public override func draggingEntered(_ sender: NSDraggingInfo) -> NSDragOperation {
        return .copy
    }
    
    public override func performDragOperation(_ sender: NSDraggingInfo) -> Bool {
        return importFromPasteboard(sender.draggingPasteboard, isDropOperation: true)
    }
    
    public override func mouseDragged(with event: NSEvent) {
        if window?.firstResponder != self {
            return
        }

        let local = viewCoordinates(event)
        mouse_moved(wsHandle, Float(local.x), Float(local.y))
        setNeedsDisplay(self.frame)
    }
    
    public override func mouseMoved(with event: NSEvent) {
        if window?.firstResponder != self {
            return
        }

        let local = viewCoordinates(event)
        mouse_moved(wsHandle, Float(local.x), Float(local.y))
        setNeedsDisplay(self.frame)
    }
    
    public override func mouseDown(with event: NSEvent) {
        let local = viewCoordinates(event)
        mouse_button(wsHandle, Float(local.x), Float(local.y), true, true, event.modifierFlags.contains(.shift), event.modifierFlags.contains(.control), event.modifierFlags.contains(.option), event.modifierFlags.contains(.command))
        setNeedsDisplay(self.frame)
    }
    
    public override func mouseUp(with event: NSEvent) {
        let local = viewCoordinates(event)
        mouse_button(wsHandle, Float(local.x), Float(local.y), false, true, event.modifierFlags.contains(.shift), event.modifierFlags.contains(.control), event.modifierFlags.contains(.option), event.modifierFlags.contains(.command))
        setNeedsDisplay(self.frame)
    }
    
    public override func rightMouseDown(with event: NSEvent) {
        let local = viewCoordinates(event)
        mouse_button(wsHandle, Float(local.x), Float(local.y), true, false, event.modifierFlags.contains(.shift), event.modifierFlags.contains(.control), event.modifierFlags.contains(.option), event.modifierFlags.contains(.command))
        setNeedsDisplay(self.frame)
    }
    
    public override func rightMouseUp(with event: NSEvent) {
        let local = viewCoordinates(event)
        mouse_button(wsHandle, Float(local.x), Float(local.y), false, false, event.modifierFlags.contains(.shift), event.modifierFlags.contains(.control), event.modifierFlags.contains(.option), event.modifierFlags.contains(.command))
        setNeedsDisplay(self.frame)
    }
    
    public override func scrollWheel(with event: NSEvent) {
        scroll_wheel(wsHandle, Float(event.scrollingDeltaY))
        setNeedsDisplay(self.frame)
    }
    
    public override func keyDown(with event: NSEvent) {
        setClipboard()
        if pasteImageInClipboard(event) {
            return
        }
        
        key_event(wsHandle, event.keyCode, event.modifierFlags.contains(.shift), event.modifierFlags.contains(.control), event.modifierFlags.contains(.option), event.modifierFlags.contains(.command), true, event.characters)
        setNeedsDisplay(self.frame)
    }
    
    public override func viewDidChangeEffectiveAppearance() {
        dark_mode(wsHandle, isDarkMode())
        setNeedsDisplay(self.frame)
    }
    
    // https://stackoverflow.com/a/53218688/1060955
    func isDarkMode() -> Bool {
        self.effectiveAppearance.bestMatch(from: [.darkAqua, .aqua]) == .darkAqua
    }
    
    public override func keyUp(with event: NSEvent) {
        key_event(wsHandle, event.keyCode, event.modifierFlags.contains(.shift), event.modifierFlags.contains(.control), event.modifierFlags.contains(.option), event.modifierFlags.contains(.command), false, event.characters)
        setNeedsDisplay(self.frame)
    }
    
    func setClipboard(){
        let pasteboardString: String? = NSPasteboard.general.string(forType: .string)
        if let theString = pasteboardString {
            system_clipboard_changed(wsHandle, theString)
        }
        self.pasteBoardEventId = NSPasteboard.general.changeCount
    }
    
    func importFromPasteboard(_ pasteBoard: NSPasteboard, isDropOperation: Bool) -> Bool {
        if let data = pasteBoard.data(forType: .png) ?? pasteBoard.data(forType: .tiff),
           let url = createTempDir() {
            let imageUrl = url.appendingPathComponent(String(UUID().uuidString.prefix(10).lowercased()), conformingTo: .png)
            
            do {
                try data.write(to: imageUrl)
            } catch {
                return false
            }
            
//            if let lbImageURL = workspaceState!.importFile(imageUrl) {
//                paste_text(wsHandle, lbImageURL)
//                workspaceState?.pasted = true
//                
//                return true
//            }
        } else if isDropOperation {
            if let data = pasteBoard.data(forType: .fileURL) {
                if let url = URL(dataRepresentation: data, relativeTo: nil) {
//                    if let markdownURL = workspaceState!.importFile(url) {
//                        paste_text(wsHandle, markdownURL)
//                        workspaceState?.pasted = true
//                        
//                        return true
//                    }
                }
            } else if let data = pasteBoard.data(forType: .string) {
                paste_text(wsHandle, String(data: data, encoding: .utf8))
                workspaceState?.pasted = true
                
                return true
            }
        }
        
        return false
    }
    
    func pasteImageInClipboard(_ event: NSEvent) -> Bool {
        if event.keyCode == 9
            && event.modifierFlags.contains(.command) {
            return importFromPasteboard(NSPasteboard.general, isDropOperation: false)
        }
        
        return false
    }

    func textFromPtr(s: UnsafeMutablePointer<CChar>) -> String {
        let str = String(cString: s)
        free_text(s)
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
        resize_editor(wsHandle, Float(size.width), Float(size.height), Float(scale))
    }
    
    public func draw(in view: MTKView) {
        if NSPasteboard.general.changeCount != self.pasteBoardEventId {
            setClipboard()
        }
        
        let scale = Float(self.window?.backingScaleFactor ?? 1.0)
        dark_mode(wsHandle, isDarkMode())
        set_scale(wsHandle, scale)
        let output = draw_editor(wsHandle)
        
        let selectedFile = UUID(uuid: output.workspace_resp.selected_file._0)
        if !selectedFile.isNil() {
            if selectedFile != self.workspaceState?.openDoc {
                self.workspaceState?.openDoc = selectedFile
            }
        }
        
        if let openedUrl = output.url_opened {
            let url = textFromPtr(s: openedUrl)
            
            if let url = URL(string: url) {
                NSWorkspace.shared.open(url)
            }

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
        
        if let text = output.copied_text {
            let text = textFromPtr(s: text)
            NSPasteboard.general.clearContents()
            NSPasteboard.general.setString(text, forType: .string)
        }
        
        let cursor = NSCursor.fromCCursor(c: output.cursor)
        print(cursor)
        if cursor != lastCursor {
            self.lastCursor = cursor
            self.resetCursorRects()
        }
    }
}
#endif

extension UUID {
    func isNil() -> Bool {
        self.uuid.0 == 0 &&
        self.uuid.1 == 0 &&
        self.uuid.2 == 0 &&
        self.uuid.3 == 0 &&
        self.uuid.4 == 0 &&
        self.uuid.5 == 0 &&
        self.uuid.6 == 0 &&
        self.uuid.7 == 0 &&
        self.uuid.8 == 0 &&
        self.uuid.9 == 0 &&
        self.uuid.10 == 0 &&
        self.uuid.11 == 0 &&
        self.uuid.12 == 0 &&
        self.uuid.13 == 0 &&
        self.uuid.14 == 0 &&
        self.uuid.15 == 0
    }
}
