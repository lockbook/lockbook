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

    var modifierEventHandle: Any? = nil

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
        drawImmediately()
    }

    func requestSync() {
        request_sync(wsHandle)
        setNeedsDisplay(self.frame)
    }
    
    func closeDoc(id: UUID) {
        close_tab(wsHandle, id.uuidString)
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

        modifierEventHandle = NSEvent.addLocalMonitorForEvents(matching: .flagsChanged, handler: modifiersChanged(event:))
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

    public override func mouseExited(with event: NSEvent) {
        mouse_gone(wsHandle)
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
        if event.hasPreciseScrollingDeltas {
            scroll_wheel(wsHandle, Float(event.scrollingDeltaX), Float(event.scrollingDeltaY), event.modifierFlags.contains(.shift), event.modifierFlags.contains(.control), event.modifierFlags.contains(.option), event.modifierFlags.contains(.command))
        } else {
            scroll_wheel(wsHandle, Float(event.scrollingDeltaX * 10), Float(event.scrollingDeltaY * 10), event.modifierFlags.contains(.shift), event.modifierFlags.contains(.control), event.modifierFlags.contains(.option), event.modifierFlags.contains(.command))
        }
        setNeedsDisplay(self.frame)
    }

    public override func magnify(with event: NSEvent) {
        magnify_gesture(wsHandle, Float(event.magnification))
        setNeedsDisplay(self.frame)
    }

    public override func keyDown(with event: NSEvent) {
        sendKeyEvent(event, true)
        setNeedsDisplay(self.frame)
    }
    
    public override func keyUp(with event: NSEvent) {
        sendKeyEvent(event, false)
        setNeedsDisplay(self.frame)
    }
    
    public override func performKeyEquivalent(with event: NSEvent) -> Bool {
        if event.modifierFlags.contains(.command) && event.keyCode == 9 {
            let _ = importFromPasteboard(NSPasteboard.general, isPaste: true)
            setNeedsDisplay(self.frame)

            return true
        } else if event.modifierFlags.contains(.command) && event.keyCode == 13 {
            sendKeyEvent(event, true)
            setNeedsDisplay(self.frame)
            
            return true
        }

        return super.performKeyEquivalent(with: event)
    }
    
    func sendKeyEvent(_ event: NSEvent, _ isDownPress: Bool) {
        let text = event.characters ?? ""
        
        key_event(wsHandle, event.keyCode, event.modifierFlags.contains(.shift), event.modifierFlags.contains(.control), event.modifierFlags.contains(.option), event.modifierFlags.contains(.command), isDownPress, text)
    }

    public override func viewDidChangeEffectiveAppearance() {
        dark_mode(wsHandle, isDarkMode())
        setNeedsDisplay(self.frame)
    }

    // https://stackoverflow.com/a/53218688/1060955
    func isDarkMode() -> Bool {
        self.effectiveAppearance.bestMatch(from: [.darkAqua, .aqua]) == .darkAqua
    }

    func setClipboard(){
        pasteboardString = NSPasteboard.general.string(forType: .string)
        self.pasteBoardEventId = NSPasteboard.general.changeCount
    }

    func pasteText(text: String) {
        clipboard_paste(wsHandle, text)
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
                    if isSupportedImageFormat(ext: url.pathExtension.lowercased()) {
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

    // copy of workspace::tab::image_viewer::is_supported_image_fmt() because ffi seems like overkill
    func isSupportedImageFormat(ext: String) -> Bool {
        // Complete list derived from which features are enabled on image crate according to image-rs default features:
        // https://github.com/image-rs/image/blob/main/Cargo.toml#L70
        let imgFormats: Set<String> = [
            "avif", "bmp", "dds", "exr", "ff", "gif", "hdr", "ico", "jpeg", "jpg", "png", "pnm", "qoi", "tga",
            "tiff", "webp"
        ]
        return imgFormats.contains(ext.lowercased())
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

    public func drawImmediately() {
        redrawTask?.cancel()
        redrawTask = nil

        self.isPaused = true
        self.enableSetNeedsDisplay = false

        self.draw(in: self)
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
        let output = macos_frame(wsHandle)

        if output.status_updated {
            let status = get_status(wsHandle)
            let msg = String(cString: status.msg)
            free_text(status.msg)
            let syncing = status.syncing
            workspaceState?.syncing = syncing
            workspaceState?.statusMsg = msg
        }

        workspaceState?.reloadFiles = output.refresh_files

        let selectedFile = UUID(uuid: output.selected_file._0)
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

        let newFile = UUID(uuid: output.doc_created._0)
        if !newFile.isNil() {
            openFile(id: newFile)
        }

        if let openedUrl = output.url_opened {
            let url = textFromPtr(s: openedUrl)

            if let url = URL(string: url) {
                self.workspaceState?.urlOpened = url
            }
        }

        if output.new_folder_btn_pressed {
            workspaceState?.newFolderButtonPressed = true
        }

        if let text = output.copied_text {
            let text = textFromPtr(s: text)
            if !text.isEmpty {
                NSPasteboard.general.clearContents()
                NSPasteboard.general.setString(text, forType: .string)
            }
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
        } else if output.cursor != Default {
            if cursorHidden {
                NSCursor.unhide()
                cursorHidden = false
            }
        }

        if output.request_paste {
            importFromPasteboard(NSPasteboard.general, isPaste: true)
            setNeedsDisplay(self.frame)
        }

        redrawTask?.cancel()
        redrawTask = nil
        self.isPaused = output.redraw_in > 50
        if self.isPaused {
            let redrawIn = UInt64(truncatingIfNeeded: output.redraw_in)
            let redrawInInterval = DispatchTimeInterval.milliseconds(Int(truncatingIfNeeded: min(500, redrawIn)));

            let newRedrawTask = DispatchWorkItem {
                self.setNeedsDisplay(self.frame)
            }
            DispatchQueue.main.asyncAfter(deadline: .now() + redrawInInterval, execute: newRedrawTask)
            redrawTask = newRedrawTask
        }

        self.enableSetNeedsDisplay = self.isPaused
    }

    deinit {
        if let wsHandle = wsHandle {
            deinit_editor(wsHandle)
        }

        if let modifierEventHandle = modifierEventHandle {
            NSEvent.removeMonitor(modifierEventHandle)
        }
    }
}

#endif
