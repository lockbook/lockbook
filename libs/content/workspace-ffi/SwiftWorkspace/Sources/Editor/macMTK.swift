#if os(macOS)
import MetalKit
import Bridge

public class MacMTK: MTKView, MTKViewDelegate {

    var wsHandle: UnsafeMutableRawPointer?
    var coreHandle: UnsafeMutableRawPointer?
    var trackingArea : NSTrackingArea?
    var pasteBoardEventId: Int = 0
    var pasteboardString: String?

    var workspaceState: WorkspaceState?

    // todo this will probably just become us hanging on to the last output
    var currentOpenDoc: UUID? = nil
    var currentSelectedFolder: UUID? = nil

    var redrawTask: DispatchWorkItem? = nil

    var lastCursor: NSCursor = NSCursor.arrow
    var cursorHidden: Bool = false

    override init(frame frameRect: CGRect, device: MTLDevice?) {
        super.init(frame: frameRect, device: device)
        self.preferredFramesPerSecond = 120
        self.delegate = self
        self.isPaused = true
        self.enableSetNeedsDisplay = true
    }

    func openFile(id: UUID) {
        let uuid = CUuid(_0: id.uuid)
        open_file(wsHandle, uuid, false)
        setNeedsDisplay(self.frame)
    }

    func requestSync() {
        request_sync(wsHandle)
        setNeedsDisplay(self.frame)
    }

    func fileOpCompleted(fileOp: WSFileOpCompleted) {
        switch fileOp {
        case .Delete(let id):
            workspaceState?.openDoc = nil
            currentOpenDoc = nil
            close_tab(wsHandle, id.uuidString)
            setNeedsDisplay(self.frame)
        case .Rename(let id, let newName):
            tab_renamed(wsHandle, id.uuidString, newName)
            setNeedsDisplay(self.frame)
        }
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
        return importFromPasteboard(sender.draggingPasteboard, isPaste: false)
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
        scroll_wheel(wsHandle, Float(event.scrollingDeltaX), Float(event.scrollingDeltaY))
        setNeedsDisplay(self.frame)
    }
    
    public override func magnify(with event: NSEvent) {
        magnify_gesture(wsHandle, Float(event.magnification))
        setNeedsDisplay(self.frame)
    }

    public override func keyDown(with event: NSEvent) {
        if event.modifierFlags.contains(.command) && event.keyCode == 9 { // cmd+v
            importFromPasteboard(NSPasteboard.general, isPaste: true)
        } else {
            key_event(wsHandle, event.keyCode, event.modifierFlags.contains(.shift), event.modifierFlags.contains(.control), event.modifierFlags.contains(.option), event.modifierFlags.contains(.command), true, event.characters)
        }

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
        pasteboardString = NSPasteboard.general.string(forType: .string)
        self.pasteBoardEventId = NSPasteboard.general.changeCount
    }

    func pasteText(text: String) {
        paste_text(wsHandle, text)
        workspaceState?.pasted = true
    }

    func importFromPasteboard(_ pasteBoard: NSPasteboard, isPaste: Bool) -> Bool {
        if let data = pasteBoard.data(forType: .png) {
            sendImage(img: data, isPaste: isPaste)
            return true
        } else if let data = pasteBoard.data(forType: .string) {
            if let text = String(data: data, encoding: .utf8) {
                clipboard_paste(wsHandle, text)
            }
        } else if !isPaste {
            if let data = pasteBoard.data(forType: .fileURL) {
                if let url = URL(dataRepresentation: data, relativeTo: nil) {
                    if url.pathExtension.lowercased() == "png" {
                        guard let data = try? Data(contentsOf: url) else {
                            return false
                        }
                        
                        sendImage(img: data, isPaste: isPaste)
                    } else {
                        clipboard_send_file(wsHandle, url.path(percentEncoded: false), isPaste)
                    }
                }
            } else if let data = pasteBoard.data(forType: .string) {
                if let text = String(data: data, encoding: .utf8) {
                    pasteText(text: text)
                    
                    return true
                }
            }
        }

        return true
    }
    
    func sendImage(img: Data, isPaste: Bool) {
        let imgPtr = img.withUnsafeBytes { (pointer: UnsafeRawBufferPointer) -> UnsafePointer<UInt8> in
            return pointer.baseAddress!.assumingMemoryBound(to: UInt8.self)
        }
        
        clipboard_send_image(wsHandle, imgPtr, UInt(img.count), isPaste)
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
        resize_editor(wsHandle, Float(size.width), Float(size.height), Float(scale))
    }

    public func draw(in view: MTKView) {
        if NSPasteboard.general.changeCount != self.pasteBoardEventId {
            setClipboard()
        }

        switch self.workspaceState?.selectedFolder{
        case .none:
            no_folder_selected(wsHandle)
        case .some(let f):
            folder_selected(wsHandle, CUuid(_0: f.uuid))
        }

        let scale = Float(self.window?.backingScaleFactor ?? 1.0)
        dark_mode(wsHandle, isDarkMode())
        set_scale(wsHandle, scale)
        let output = draw_editor(wsHandle)
        
        workspaceState?.syncing = output.workspace_resp.syncing
        workspaceState?.statusMsg = textFromPtr(s: output.workspace_resp.msg)
        workspaceState?.reloadFiles = output.workspace_resp.refresh_files

        let selectedFile = UUID(uuid: output.workspace_resp.selected_file._0)
        if !selectedFile.isNil() {
            currentOpenDoc = selectedFile
            if selectedFile != self.workspaceState?.openDoc {
                self.workspaceState?.openDoc = selectedFile
            }
        }
        
        let currentTab = WorkspaceTab(rawValue: Int(current_tab(wsHandle)))!
        if currentTab == .Welcome && currentOpenDoc != nil {
            currentOpenDoc = nil
            self.workspaceState?.openDoc = nil
        }

        let newFile = UUID(uuid: output.workspace_resp.doc_created._0)
        if !newFile.isNil() {
            self.workspaceState?.openDoc = newFile
        }

        if let openedUrl = output.url_opened {
            let url = textFromPtr(s: openedUrl)

            if let url = URL(string: url) {
                NSWorkspace.shared.open(url)
            }
        }

        if output.workspace_resp.new_folder_btn_pressed {
            workspaceState?.newFolderButtonPressed = true
        }

        redrawTask?.cancel()
        self.isPaused = output.redraw_in > 100
        if self.isPaused {
            let redrawIn = UInt64(truncatingIfNeeded: output.redraw_in)
            let redrawInInterval = DispatchTimeInterval.milliseconds(Int(truncatingIfNeeded: min(500, redrawIn)));
            
            let newRedrawTask = DispatchWorkItem {
                self.setNeedsDisplay(self.frame)
            }
            DispatchQueue.main.asyncAfter(deadline: .now() + redrawInInterval, execute: newRedrawTask)
            redrawTask = newRedrawTask
        }

        if let text = output.copied_text {
            let text = textFromPtr(s: text)
            NSPasteboard.general.clearContents()
            NSPasteboard.general.setString(text, forType: .string)
        }
        
        let cursor = NSCursor.fromCCursor(c: output.cursor)
        if cursor != lastCursor {
            self.lastCursor = cursor
            self.resetCursorRects()
        }
        
        if output.cursor == None {
            if !cursorHidden {
                NSCursor.hide()
                cursorHidden = true
            }
        } else {
            if cursorHidden {
                NSCursor.unhide()
                cursorHidden = false
            }
        }
    }
}

#endif
