#if os(macOS)
    import Bridge
    import MetalKit

    public class MacMTK: MTKView, MTKViewDelegate {
        var wsHandle: UnsafeMutableRawPointer?
        var coreHandle: UnsafeMutableRawPointer?
        var trackingArea: NSTrackingArea?
        var pasteBoardEventId: Int = 0
        var pasteboardString: String?

        var workspaceInput: WorkspaceInputState?
        var workspaceOutput: WorkspaceOutputState?

        /// todo this will probably just become us hanging on to the last output
        var currentOpenDoc: UUID?

        var redrawTask: DispatchWorkItem?

        var lastCursor: NSCursor = .arrow
        var cursorHidden: Bool = false

        var modifierEventHandle: Any?

        override init(frame frameRect: CGRect, device: MTLDevice?) {
            super.init(frame: frameRect, device: device)
            preferredFramesPerSecond = 120
            delegate = self
            isPaused = true
            enableSetNeedsDisplay = true
        }

        func modifiersChanged(event: NSEvent) -> NSEvent {
            modifier_event(wsHandle, event.modifierFlags.contains(.shift), event.modifierFlags.contains(.control), event.modifierFlags.contains(.option), event.modifierFlags.contains(.command))
            setNeedsDisplay(frame)
            return event
        }

        override public func resetCursorRects() {
            addCursorRect(frame, cursor: lastCursor)
        }

        override public func updateTrackingAreas() {
            if trackingArea != nil {
                removeTrackingArea(trackingArea!)
            }
            let options: NSTrackingArea.Options =
                [.mouseEnteredAndExited, .mouseMoved, .enabledDuringMouseDrag, .activeInKeyWindow]
            trackingArea = NSTrackingArea(rect: bounds, options: options,
                                          owner: self, userInfo: nil)
            addTrackingArea(trackingArea!)
        }

        @available(*, unavailable)
        required init(coder _: NSCoder) {
            fatalError("init(coder:) has not been implemented")
        }

        override public var acceptsFirstResponder: Bool {
            true
        }

        override public func viewDidMoveToWindow() {
            super.viewDidMoveToWindow()
            window?.makeFirstResponder(self)
        }

        public func setInitialContent(_ coreHandle: UnsafeMutableRawPointer?) {
            let metalLayer = UnsafeMutableRawPointer(Unmanaged.passUnretained(layer!).toOpaque())
            wsHandle = init_ws(coreHandle, metalLayer, isDarkMode(), true)
            workspaceInput?.wsHandle = wsHandle

            modifierEventHandle = NSEvent.addLocalMonitorForEvents(matching: .flagsChanged, handler: modifiersChanged(event:))
            registerForDraggedTypes([.png, .tiff, .fileURL, .string])
            becomeFirstResponder()
        }

        override public func draggingEntered(_: NSDraggingInfo) -> NSDragOperation {
            .copy
        }

        override public func performDragOperation(_ sender: NSDraggingInfo) -> Bool {
            importFromPasteboard(sender.draggingPasteboard, isPaste: false)
        }

        override public func mouseDragged(with event: NSEvent) {
            if window?.firstResponder != self {
                return
            }

            let local = viewCoordinates(event)
            mouse_moved(wsHandle, Float(local.x), Float(local.y))
            setNeedsDisplay(frame)
        }

        override public func mouseMoved(with event: NSEvent) {
            if window?.firstResponder != self {
                return
            }

            let local = viewCoordinates(event)
            mouse_moved(wsHandle, Float(local.x), Float(local.y))
            setNeedsDisplay(frame)
        }

        override public func mouseExited(with _: NSEvent) {
            mouse_gone(wsHandle)
            setNeedsDisplay(frame)
        }

        override public func mouseDown(with event: NSEvent) {
            let local = viewCoordinates(event)
            mouse_button(wsHandle, Float(local.x), Float(local.y), true, true, event.modifierFlags.contains(.shift), event.modifierFlags.contains(.control), event.modifierFlags.contains(.option), event.modifierFlags.contains(.command))
            setNeedsDisplay(frame)
        }

        override public func mouseUp(with event: NSEvent) {
            let local = viewCoordinates(event)
            mouse_button(wsHandle, Float(local.x), Float(local.y), false, true, event.modifierFlags.contains(.shift), event.modifierFlags.contains(.control), event.modifierFlags.contains(.option), event.modifierFlags.contains(.command))
            setNeedsDisplay(frame)
        }

        override public func rightMouseDown(with event: NSEvent) {
            let local = viewCoordinates(event)
            mouse_button(wsHandle, Float(local.x), Float(local.y), true, false, event.modifierFlags.contains(.shift), event.modifierFlags.contains(.control), event.modifierFlags.contains(.option), event.modifierFlags.contains(.command))
            setNeedsDisplay(frame)
        }

        override public func rightMouseUp(with event: NSEvent) {
            let local = viewCoordinates(event)
            mouse_button(wsHandle, Float(local.x), Float(local.y), false, false, event.modifierFlags.contains(.shift), event.modifierFlags.contains(.control), event.modifierFlags.contains(.option), event.modifierFlags.contains(.command))
            setNeedsDisplay(frame)
        }

        override public func scrollWheel(with event: NSEvent) {
            if event.hasPreciseScrollingDeltas {
                scroll_wheel(wsHandle, Float(event.scrollingDeltaX), Float(event.scrollingDeltaY), event.modifierFlags.contains(.shift), event.modifierFlags.contains(.control), event.modifierFlags.contains(.option), event.modifierFlags.contains(.command))
            } else {
                scroll_wheel(wsHandle, Float(event.scrollingDeltaX * 10), Float(event.scrollingDeltaY * 10), event.modifierFlags.contains(.shift), event.modifierFlags.contains(.control), event.modifierFlags.contains(.option), event.modifierFlags.contains(.command))
            }
            setNeedsDisplay(frame)
        }

        override public func magnify(with event: NSEvent) {
            magnify_gesture(wsHandle, Float(event.magnification))
            setNeedsDisplay(frame)
        }

        override public func keyDown(with event: NSEvent) {
            sendKeyEvent(event, true)
        }

        override public func keyUp(with event: NSEvent) {
            sendKeyEvent(event, false)
        }

        override public func performKeyEquivalent(with event: NSEvent) -> Bool {
            // Let system handle their shortcuts first
            if super.performKeyEquivalent(with: event) {
                return true
            }

            guard event.modifierFlags.contains(.command) else {
                return false
            }

            switch event.keyCode {
            case 9: // V key
                // If first responder isn't a text editor then we hijack the paste
                if !(window?.firstResponder is NSTextView) {
                    _ = importFromPasteboard(NSPasteboard.general, isPaste: true)
                    setNeedsDisplay(frame)
                    return true
                }

                return false

            case 13: // Return key
                sendKeyEvent(event, true)
                return true

            default:
                return false
            }
        }

        func sendKeyEvent(_ event: NSEvent, _ isDownPress: Bool) {
            let text = event.characters ?? ""

            key_event(wsHandle, event.keyCode, event.modifierFlags.contains(.shift), event.modifierFlags.contains(.control), event.modifierFlags.contains(.option), event.modifierFlags.contains(.command), isDownPress, text)

            setNeedsDisplay(frame)
        }

        override public func viewDidChangeEffectiveAppearance() {
            dark_mode(wsHandle, isDarkMode())
            setNeedsDisplay(frame)
        }

        /// https://stackoverflow.com/a/53218688/1060955
        func isDarkMode() -> Bool {
            effectiveAppearance.bestMatch(from: [.darkAqua, .aqua]) == .darkAqua
        }

        func setClipboard() {
            pasteboardString = NSPasteboard.general.string(forType: .string)
            pasteBoardEventId = NSPasteboard.general.changeCount
        }

        func pasteText(text: String) {
            clipboard_paste(wsHandle, text)
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

        /// copy of workspace::tab::image_viewer::is_supported_image_fmt() because ffi seems like overkill
        func isSupportedImageFormat(ext: String) -> Bool {
            // Complete list derived from which features are enabled on image crate according to image-rs default features:
            // https://github.com/image-rs/image/blob/main/Cargo.toml#L70
            let imgFormats: Set = [
                "avif", "bmp", "dds", "exr", "ff", "gif", "hdr", "ico", "jpeg", "jpg", "png", "pnm", "qoi", "tga",
                "tiff", "webp",
            ]
            return imgFormats.contains(ext.lowercased())
        }

        func viewCoordinates(_ event: NSEvent) -> NSPoint {
            var local = convert(event.locationInWindow, from: nil)
            local.y = frame.size.height - local.y
            return local
        }

        public func mtkView(_: MTKView, drawableSizeWillChange size: CGSize) {
            // initially window is not set, this defaults to 1.0, initial frame comes from `init_editor`
            // we probably want a setNeedsDisplay here
            let scale = window?.backingScaleFactor ?? 1.0
            resize_editor(wsHandle, Float(size.width), Float(size.height), Float(scale))
        }

        public func drawImmediately() {
            redrawTask?.cancel()
            redrawTask = nil

            isPaused = true
            enableSetNeedsDisplay = false

            draw(in: self)
        }

        public func draw(in _: MTKView) {
            if NSPasteboard.general.changeCount != pasteBoardEventId {
                setClipboard()
            }

            let scale = Float(window?.backingScaleFactor ?? 1.0)
            dark_mode(wsHandle, isDarkMode())
            set_scale(wsHandle, scale)
            let output = macos_frame(wsHandle)

            if output.selected_folder_changed {
                let selectedFolder = UUID(uuid: get_selected_folder(wsHandle)._0)
                if selectedFolder.isNil() {
                    workspaceOutput?.selectedFolder = nil
                } else {
                    workspaceOutput?.selectedFolder = selectedFolder
                }
            }

            let selectedFile = UUID(uuid: output.selected_file._0)
            if !selectedFile.isNil() {
                currentOpenDoc = selectedFile
                if selectedFile != workspaceOutput?.openDoc {
                    workspaceOutput?.openDoc = selectedFile
                }
            }

            let currentTab = WorkspaceTab(rawValue: Int(current_tab(wsHandle)))!
            if currentTab == .Welcome, currentOpenDoc != nil {
                currentOpenDoc = nil
                workspaceOutput?.openDoc = nil
            }

//      FIXME: Can we just do this in rust?
            let newFile = UUID(uuid: output.doc_created._0)
            if !newFile.isNil() {
                workspaceInput?.openFile(id: newFile)
            }

            if output.urls_opened.size > 0 {
                var urls: [URL] = []
                for i in 0 ..< Int(output.urls_opened.size) {
                    // Don't use textFromPtr here — it frees each string, but
                    // free_urls below frees the strings and the array together.
                    if let ptr = output.urls_opened.urls[i], let url = URL(string: String(cString: ptr)) {
                        urls.append(url)
                    }
                }
                workspaceOutput?.urlsOpened = urls
                free_urls(output.urls_opened)
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
                lastCursor = cursor
                resetCursorRects()
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
                setNeedsDisplay(frame)
            }

            redrawTask?.cancel()
            redrawTask = nil
            isPaused = output.redraw_in > 50
            if isPaused {
                let redrawIn = UInt64(truncatingIfNeeded: output.redraw_in)
                let redrawInInterval = DispatchTimeInterval.milliseconds(Int(truncatingIfNeeded: min(500, redrawIn)))

                let newRedrawTask = DispatchWorkItem {
                    self.setNeedsDisplay(self.frame)
                }
                DispatchQueue.main.asyncAfter(deadline: .now() + redrawInInterval, execute: newRedrawTask)
                redrawTask = newRedrawTask
            }

            enableSetNeedsDisplay = isPaused
        }

        deinit {
            if let wsHandle {
                deinit_editor(wsHandle)
            }

            if let modifierEventHandle {
                NSEvent.removeMonitor(modifierEventHandle)
            }
        }
    }


extension MacMTK: NSTextInputClient {
    // Called by the macOS Character Viewer (fn key) and other text input services.
    public func insertText(_ string: Any, replacementRange _: NSRange) {
        let text: String
        if let attr = string as? NSAttributedString { text = attr.string }
        else if let str = string as? String { text = str }
        else { return }
        clipboard_paste(wsHandle, text)
        setNeedsDisplay(self.frame)
    }
    public func setMarkedText(_ string: Any, selectedRange _: NSRange, replacementRange _: NSRange) {}
    public func unmarkText() {}
    public func selectedRange() -> NSRange { NSRange(location: NSNotFound, length: 0) }
    public func markedRange() -> NSRange { NSRange(location: NSNotFound, length: 0) }
    public func hasMarkedText() -> Bool { false }
    public func attributedSubstring(forProposedRange _: NSRange, actualRange _: NSRangePointer?) -> NSAttributedString? { nil }
    public func validAttributesForMarkedText() -> [NSAttributedString.Key] { [] }
    public func firstRect(forCharacterRange _: NSRange, actualRange _: NSRangePointer?) -> NSRect {
        window?.convertToScreen(frame) ?? .zero
    }
    public func characterIndex(for _: NSPoint) -> Int { NSNotFound }
}
#endif
