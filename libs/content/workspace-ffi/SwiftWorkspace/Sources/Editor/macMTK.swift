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
        self.preferredFramesPerSecond = 120
        self.delegate = self
        self.isPaused = true
        self.enableSetNeedsDisplay = true
    }

    func openFile(id: UUID) {
        if currentOpenDoc != id {
            let uuid = CUuid(_0: id.uuid)
            open_file(wsHandle, uuid, false)
            setNeedsDisplay(self.frame)
        }

    }

    func requestSync() {
        withUnsafeMutablePointer(to: &workspaceState) { workspaceStatePtr in
            request_sync(wsHandle, workspaceStatePtr, updateSyncMessage)
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
        scroll_wheel(wsHandle, Float(event.scrollingDeltaX), Float(event.scrollingDeltaY))
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

    func importImageData(data: Data, isPNG: Bool) -> Bool {
        guard let url = createTempDir() else {
            return false
        }

        let imageUrl = url.appendingPathComponent(String(UUID().uuidString.prefix(10).lowercased()), conformingTo: isPNG ? .png : .tiff)

        do {
            try data.write(to: imageUrl)

            if let lbImageURL = workspaceState!.importFile(imageUrl) {
                pasteText(text: lbImageURL)

                return true
            }
        } catch {}

        return false
    }

    func pasteText(text: String) {
        paste_text(wsHandle, text)
        workspaceState?.pasted = true
    }

    func importFromPasteboard(_ pasteBoard: NSPasteboard, isDropOperation: Bool) -> Bool {
        if let data = pasteBoard.data(forType: .png) {
            return importImageData(data: data, isPNG: true)
        } else if let data = pasteBoard.data(forType: .tiff) {
            return importImageData(data: data, isPNG: false)
        } else if isDropOperation {
            if let data = pasteBoard.data(forType: .fileURL) {
                if let url = URL(dataRepresentation: data, relativeTo: nil) {
                    if let markdownURL = workspaceState!.importFile(url) {
                        pasteText(text: markdownURL)

                        return true
                    }
                }
            } else if let data = pasteBoard.data(forType: .string) {
                if let text = String(data: data, encoding: .utf8) {
                    pasteText(text: text)

                    return true
                }
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
        if !output.workspace_resp.syncing { // sync closure will populate status message
            workspaceState?.statusMsg = textFromPtr(s: output.workspace_resp.msg)
        }
        workspaceState?.reloadFiles = output.workspace_resp.refresh_files

        let selectedFile = UUID(uuid: output.workspace_resp.selected_file._0)
        if selectedFile.isNil() {
            currentOpenDoc = nil
            if self.workspaceState?.openDoc != nil {
                self.workspaceState?.openDoc = nil
            }
        } else {
            currentOpenDoc = selectedFile
            if selectedFile != self.workspaceState?.openDoc {
                self.workspaceState?.openDoc = selectedFile
            }
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
            var redrawIn = Int(truncatingIfNeeded: output.redraw_in)
            if redrawIn == -1 {
                // todo: this means that at a mimumum we're going to trigger 1 frame per second
                // so that long running background tasks within egui eventually have their status
                // shown. Ideally this would get replaced by some form of trigger within egui
                // (what happens if requestRepaint is called while no frame is being drawn)
                redrawIn = 1000
            }

            let newRedrawTask = DispatchWorkItem {
                self.setNeedsDisplay(self.frame)
            }
            DispatchQueue.main.asyncAfter(deadline: .now() + .milliseconds(redrawIn), execute: newRedrawTask)
            redrawTask = newRedrawTask
        }

        if let text = output.copied_text {
            let text = textFromPtr(s: text)
            NSPasteboard.general.clearContents()
            NSPasteboard.general.setString(text, forType: .string)
        }
        
        if output.cursor == None {
            NSCursor.hide()
        } else {
            let cursor = NSCursor.fromCCursor(c: output.cursor)
            if cursor != lastCursor {
                self.lastCursor = cursor
                self.resetCursorRects()
            }
        }
    }
}

#endif
