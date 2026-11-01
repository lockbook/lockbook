#if os(macOS)
    import Bridge
    import MetalKit
    import UniformTypeIdentifiers

    public class MacMTK: MTKView, MTKViewDelegate {
        var wsHandle: UnsafeMutableRawPointer?
        var coreHandle: UnsafeMutableRawPointer?
        var claimedPersistence = false
        var trackingArea: NSTrackingArea?
        var pasteBoardEventId: Int = 0
        var pasteboardString: String?

        var workspaceInput: WorkspaceInputState?
        var workspaceOutput: WorkspaceOutputState?

        /// todo this will probably just become us hanging on to the last output
        var currentOpenDoc: UUID?

        var redrawTask: DispatchWorkItem?
        var redrawDeadline: DispatchTime?

        var lastCursor: NSCursor = .arrow

        /// The input method's composition, while there is one.
        var imeMarked: String?
        /// The keystroke the input method is handling, whether it took it,
        /// and whether it passed the key on as well.
        var imeKeyEvent: NSEvent?
        var imeHandled = false
        var imePassKey = false
        /// The caret, in view points from the top left, for the candidate window.
        var imeCursor: NSRect = .zero
        var chromeTextFocused = false
        var cursorHidden: Bool = false

        var lastAccentColors: UInt64 = 0

        var modifierEventHandle: Any?
        var screenChangeObserver: NSObjectProtocol?
        var accentChangeObserver: NSObjectProtocol?
        var appActiveObserver: NSObjectProtocol?
        var keyWindowObservers: [NSObjectProtocol] = []
        var windowKey: Bool?

        override init(frame frameRect: CGRect, device: MTLDevice?) {
            super.init(frame: frameRect, device: device)
            preferredFramesPerSecond = 144
            delegate = self
            isPaused = true
            enableSetNeedsDisplay = true
        }

        func modifiersChanged(event: NSEvent) -> NSEvent {
            // input methods toggle languages on a bare Shift
            if window?.firstResponder === self, imeMarked != nil || imeCursor.height > 0 {
                _ = inputContext?.handleEvent(event)
            }
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
                [.mouseEnteredAndExited, .mouseMoved, .enabledDuringMouseDrag, .activeAlways]
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

            updatePreferredFrameRate()

            if let screenChangeObserver {
                NotificationCenter.default.removeObserver(screenChangeObserver)
                self.screenChangeObserver = nil
            }
            keyWindowObservers.forEach(NotificationCenter.default.removeObserver)
            keyWindowObservers = []

            if let window {
                screenChangeObserver = NotificationCenter.default.addObserver(
                    forName: NSWindow.didChangeScreenNotification,
                    object: window,
                    queue: .main
                ) { [weak self] _ in
                    self?.updatePreferredFrameRate()
                }
                // draw() reports the change
                keyWindowObservers = [NSWindow.didBecomeKeyNotification, NSWindow.didResignKeyNotification].map {
                    NotificationCenter.default.addObserver(forName: $0, object: window, queue: .main) { [weak self] _ in
                        guard let self else { return }
                        setNeedsDisplay(frame)
                    }
                }
            }
        }

        private func updatePreferredFrameRate() {
            preferredFramesPerSecond = window?.screen?.maximumFramesPerSecond ?? 60
        }

        public func setInitialContent(_ coreHandle: UnsafeMutableRawPointer?) {
            let metalLayer = UnsafeMutableRawPointer(Unmanaged.passUnretained(layer!).toOpaque())
            claimedPersistence = true
            wsHandle = init_ws(coreHandle, metalLayer, isDarkMode(), WorkspacePersistence.claim())
            workspaceInput?.wsHandle = wsHandle
            if let wsHandle {
                set_desktop_tab_policy(wsHandle, true)
            }

            if let wsHandle {
                RepaintRelay.register(wsHandle) { [weak self] delayMs in
                    DispatchQueue.main.async {
                        self?.repaintRequested(inMs: delayMs)
                    }
                }
                set_repaint_callback(wsHandle, wsHandle) { context, delayMs in
                    RepaintRelay.fire(context, delayMs)
                }
            }

            modifierEventHandle = NSEvent.addLocalMonitorForEvents(matching: .flagsChanged) { [weak self] event in
                self?.modifiersChanged(event: event) ?? event
            }
            registerForDraggedTypes([.gif, .png, .tiff, .fileURL, .string])
            becomeFirstResponder()

            accentChangeObserver = NotificationCenter.default.addObserver(
                forName: NSColor.systemColorsDidChangeNotification,
                object: nil,
                queue: .main
            ) { [weak self] _ in
                guard let self else { return }
                setNeedsDisplay(frame)
            }

            appActiveObserver = NotificationCenter.default.addObserver(
                forName: NSApplication.didBecomeActiveNotification,
                object: nil,
                queue: .main
            ) { [weak self] _ in
                self?.syncModifiers()
            }
        }

        func repaintRequested(inMs delayMs: UInt64) {
            if delayMs == 0 {
                setNeedsDisplay(frame)
            } else {
                scheduleRedraw(inMs: delayMs)
            }
        }

        func scheduleRedraw(inMs delayMs: UInt64) {
            let deadline = DispatchTime.now()
                + .milliseconds(Int(min(delayMs, UInt64(Int32.max))))

            if redrawTask != nil, let existing = redrawDeadline, existing <= deadline {
                return
            }

            redrawTask?.cancel()
            let task = DispatchWorkItem { [weak self] in
                guard let self else { return }
                setNeedsDisplay(frame)
            }
            redrawTask = task
            redrawDeadline = deadline
            DispatchQueue.main.asyncAfter(deadline: deadline, execute: task)
        }

        func syncModifiers() {
            let flags = NSEvent.modifierFlags
            modifier_event(
                wsHandle,
                flags.contains(.shift),
                flags.contains(.control),
                flags.contains(.option),
                flags.contains(.command)
            )
            setNeedsDisplay(frame)
        }

        override public func draggingEntered(_: NSDraggingInfo) -> NSDragOperation {
            .copy
        }

        override public func performDragOperation(_ sender: NSDraggingInfo) -> Bool {
            importFromPasteboard(sender.draggingPasteboard, isPaste: false)
        }

        override public func mouseDragged(with event: NSEvent) {
            let local = viewCoordinates(event)
            mouse_moved(wsHandle, Float(local.x), Float(local.y))
            setNeedsDisplay(frame)
        }

        override public func mouseMoved(with event: NSEvent) {
            let local = viewCoordinates(event)
            mouse_moved(wsHandle, Float(local.x), Float(local.y))
            setNeedsDisplay(frame)
        }

        override public func mouseExited(with _: NSEvent) {
            mouse_gone(wsHandle)
            setNeedsDisplay(frame)
        }

        override public func mouseDown(with event: NSEvent) {
            endComposition()
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
            endComposition()
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
            if event.phase == .began {
                set_trackpad_history_swipe(wsHandle, NSEvent.isSwipeTrackingFromScrollEventsEnabled)
            }
            let phase = UInt64(event.phase.rawValue)
            let momentumPhase = UInt64(event.momentumPhase.rawValue)
            if event.hasPreciseScrollingDeltas {
                scroll_wheel(wsHandle, Float(event.scrollingDeltaX), Float(event.scrollingDeltaY), phase, momentumPhase, true, event.modifierFlags.contains(.shift), event.modifierFlags.contains(.control), event.modifierFlags.contains(.option), event.modifierFlags.contains(.command))
            } else {
                scroll_wheel(wsHandle, Float(event.scrollingDeltaX * 10), Float(event.scrollingDeltaY * 10), phase, momentumPhase, false, event.modifierFlags.contains(.shift), event.modifierFlags.contains(.control), event.modifierFlags.contains(.option), event.modifierFlags.contains(.command))
            }
            setNeedsDisplay(frame)
        }

        override public func magnify(with event: NSEvent) {
            magnify_gesture(wsHandle, Float(event.magnification))
            setNeedsDisplay(frame)
        }

        override public func keyDown(with event: NSEvent) {
            // While text input is live the input method sees keystrokes
            // first; what it doesn't take (plain text, arrows, delete,
            // return) goes the usual way. Repeats skip it, so a held key
            // repeats rather than opening the accent menu.
            let composing = imeMarked != nil
            let flags = event.modifierFlags
            let routed = composing
                ? !flags.contains(.command)
                : imeCursor.height > 0 && !event.isARepeat && flags.isDisjoint(with: [.command, .control])
            if composing && !routed {
                endComposition()
            }
            if routed, let inputContext {
                imeKeyEvent = event
                imeHandled = false
                imePassKey = false
                let consumed = inputContext.handleEvent(event)
                imeKeyEvent = nil
                if (imeHandled || (composing && consumed)) && !imePassKey {
                    setNeedsDisplay(frame)
                    return
                }
            }
            sendKeyEvent(event, true)
        }

        /// Keeps a composition as typed before a shortcut or click acts, as
        /// AppKit's text views do.
        func endComposition() {
            guard imeMarked != nil else { return }
            unmarkText()
            inputContext?.discardMarkedText()
        }

        // The input method passes a key on, even after the same keystroke
        // committed text (Korean: a syllable, then the space or return).
        override public func doCommand(by _: Selector) {
            if imeKeyEvent != nil { imePassKey = true }
        }

        override public func keyUp(with event: NSEvent) {
            sendKeyEvent(event, false)
        }

        override public func performKeyEquivalent(with event: NSEvent) -> Bool {
            if event.modifierFlags.contains(.command) {
                endComposition()
            }
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
            // a spreadsheet copies cells as text and as a picture of them: the
            // text is the paste. A line beside an image names the image.
            let text = pasteBoard.string(forType: .string)
            let cells = text.map { $0.contains("\t") || $0.contains("\n") } ?? false
            if let data = pasteBoard.data(forType: .gif) ?? pasteBoard.data(forType: .png), !cells {
                sendImage(img: data, isPaste: isPaste)
                return true
            } else if let text {
                clipboard_paste(wsHandle, text)
            } else if let data = webArchiveImage(pasteBoard) {
                sendImage(img: data, isPaste: isPaste)
                return true
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

        func syncAccentColor() {
            guard let wsHandle else { return }
            guard let light = packedAccent(for: .aqua),
                  let dark = packedAccent(for: .darkAqua) else { return }

            let combined = (UInt64(light) << 32) | UInt64(dark)
            guard combined != lastAccentColors else { return }
            lastAccentColors = combined

            set_accent_color(wsHandle, light, dark)
        }

        private func packedAccent(for name: NSAppearance.Name) -> UInt32? {
            var packed: UInt32?
            NSAppearance(named: name)?.performAsCurrentDrawingAppearance {
                guard let c = NSColor.controlAccentColor.usingColorSpace(.sRGB) else { return }
                let r = UInt32((min(max(c.redComponent, 0), 1) * 255).rounded())
                let g = UInt32((min(max(c.greenComponent, 0), 1) * 255).rounded())
                let b = UInt32((min(max(c.blueComponent, 0), 1) * 255).rounded())
                packed = (r << 24) | (g << 16) | (b << 8) | 0xFF
            }
            return packed
        }

        public func mtkView(_: MTKView, drawableSizeWillChange size: CGSize) {
            // initially window is not set, this defaults to 1.0, initial frame comes from `init_editor`
            // we probably want a setNeedsDisplay here
            let scale = window?.backingScaleFactor ?? 1.0
            resize_editor(wsHandle, Float(max(size.width, 1)), Float(max(size.height, 1)), Float(scale))
        }

        public func drawImmediately() {
            redrawTask?.cancel()
            redrawTask = nil
            redrawDeadline = nil

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
            syncAccentColor()
            set_contact_linked_sites(wsHandle, UserDefaults.standard.bool(forKey: "contactLinkedSites"))
            set_open_in_new_tab(wsHandle, UserDefaults.standard.object(forKey: "openInNewTab") as? Bool ?? true)
            set_scale(wsHandle, scale)
            let key = window?.isKeyWindow ?? false
            if key != windowKey {
                windowKey = key
                window_focused(wsHandle, key)
            }

            let output = macos_frame(wsHandle)
            chromeTextFocused = output.chrome_text_focused

            let c = output.ime_cursor
            let imeRect = NSRect(x: CGFloat(c.min_x), y: CGFloat(c.min_y), width: CGFloat(c.max_x - c.min_x), height: CGFloat(c.max_y - c.min_y))
            if imeRect.height == 0, imeMarked != nil {
                // the editor dropped the composition, or lost focus
                imeMarked = nil
                inputContext?.discardMarkedText()
            }
            if imeRect != imeCursor {
                imeCursor = imeRect
                if imeMarked != nil { inputContext?.invalidateCharacterCoordinates() }
            }

            if output.tabs_changed {
                workspaceOutput?.tabCount = Int(tab_count(wsHandle))
            }

            if output.selected_folder_changed {
                let selectedFolder = UUID(uuid: get_selected_folder(wsHandle)._0)
                if selectedFolder.isNil() {
                    workspaceOutput?.selectedFolder = nil
                } else {
                    workspaceOutput?.selectedFolder = selectedFolder
                }
            }

            let selectedFile = UUID(uuid: output.selected_file._0)
            let selectedSession = UUID(uuid: output.selected_tab._0)
            // selected_file / selected_tab are one-frame events (workspace
            // output is taken each frame). Keep last-known values unless the
            // live tab type says nothing is open.
            if !selectedSession.isNil() {
                workspaceOutput?.currentSession = selectedSession
            }
            if !selectedFile.isNil() {
                currentOpenDoc = selectedFile
                if selectedFile != workspaceOutput?.openDoc {
                    workspaceOutput?.openDoc = selectedFile
                    window?.makeFirstResponder(self)
                }
            }

            let currentTab = WorkspaceTab(rawValue: Int(current_tab(wsHandle))) ?? .Welcome
            if currentTab == .Welcome {
                workspaceOutput?.currentSession = nil
            }
            let outline = currentTab == .Markdown && outline_available(wsHandle)
            if outline != workspaceOutput?.outlineAvailable {
                workspaceOutput?.outlineAvailable = outline
            }
            let outlineOpen = currentTab == .Markdown && outline_open(wsHandle)
            if outlineOpen != workspaceOutput?.outlineOpen {
                workspaceOutput?.outlineOpen = outlineOpen
            }
            if currentTab == .Welcome || currentTab == .Search, currentOpenDoc != nil {
                currentOpenDoc = nil
                workspaceOutput?.openDoc = nil
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

            if let text = output.copied_text, let html = output.copied_html {
                let text = textFromPtr(s: text)
                let html = textFromPtr(s: html)
                if !text.isEmpty {
                    NSPasteboard.general.clearContents()
                    NSPasteboard.general.setString(text, forType: .string)
                    if !html.isEmpty {
                        NSPasteboard.general.setString(html, forType: .html)
                    }
                }
            }

            if let data = dataFromBytes(b: output.copied_image), let image = NSImage(data: data) {
                NSPasteboard.general.clearContents()
                if data.starts(with: Array("GIF8".utf8)) {
                    // a gif copies as itself, with its first frame for apps that take pictures
                    let item = NSPasteboardItem()
                    item.setData(data, forType: .gif)
                    if let tiff = image.tiffRepresentation {
                        item.setData(tiff, forType: .tiff)
                    }
                    NSPasteboard.general.writeObjects([item])
                } else {
                    NSPasteboard.general.writeObjects([image])
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
            redrawDeadline = nil
            isPaused = output.redraw_in > 50
            if isPaused, output.redraw_in != UInt64.max {
                scheduleRedraw(inMs: UInt64(truncatingIfNeeded: output.redraw_in))
            }

            enableSetNeedsDisplay = isPaused
        }

        deinit {
            if let wsHandle {
                RepaintRelay.unregister(wsHandle)
                deinit_editor(wsHandle)
            }

            if claimedPersistence {
                WorkspacePersistence.release()
            }

            if let modifierEventHandle {
                NSEvent.removeMonitor(modifierEventHandle)
            }

            if let screenChangeObserver {
                NotificationCenter.default.removeObserver(screenChangeObserver)
            }
            keyWindowObservers.forEach(NotificationCenter.default.removeObserver)

            if let accentChangeObserver {
                NotificationCenter.default.removeObserver(accentChangeObserver)
            }

            if let appActiveObserver {
                NotificationCenter.default.removeObserver(appActiveObserver)
            }
        }
    }


extension MacMTK: NSTextInputClient {
    public func insertText(_ string: Any, replacementRange _: NSRange) {
        let text: String
        if let attr = string as? NSAttributedString { text = attr.string }
        else if let str = string as? String { text = str }
        else { return }
        let ownCharacter = imeKeyEvent?.characters == text && !imeHandled
        if imeMarked != nil || (imeKeyEvent != nil && !ownCharacter) {
            // A composition ends (a keystroke, or a click on a candidate),
            // or the keystroke already went to the input method.
            ime_commit(wsHandle, text)
            imeMarked = nil
            if imeKeyEvent != nil { imeHandled = true }
        } else if imeKeyEvent != nil {
            // A keystroke's own character takes the key path.
            return
        } else {
            // The Character Viewer (fn key) and other text input services.
            clipboard_paste(wsHandle, text)
        }
        setNeedsDisplay(self.frame)
    }
    public func setMarkedText(_ string: Any, selectedRange _: NSRange, replacementRange _: NSRange) {
        let text = (string as? NSAttributedString)?.string ?? (string as? String) ?? ""
        ime_preedit(wsHandle, text)
        imeMarked = text.isEmpty ? nil : text
        imeHandled = true
        setNeedsDisplay(self.frame)
    }
    public func unmarkText() {
        if let imeMarked { ime_commit(wsHandle, imeMarked) }
        imeMarked = nil
    }
    /// The editor's selection in UTF-16 units. Dictation won't start without one.
    public func selectedRange() -> NSRange {
        guard imeCursor.height > 0, !chromeTextFocused, let wsHandle else { return NSRange(location: NSNotFound, length: 0) }
        let range = get_selected(wsHandle)
        if range.none { return NSRange(location: NSNotFound, length: 0) }
        return NSRange(location: Int(range.start.pos), length: Int(range.end.pos - range.start.pos))
    }
    public func markedRange() -> NSRange {
        NSRange(location: imeMarked == nil ? NSNotFound : 0, length: imeMarked?.utf16.count ?? 0)
    }
    public func hasMarkedText() -> Bool { imeMarked != nil }
    /// Text the system reads before the caret, as dictation does for spacing and capitals.
    public func attributedSubstring(forProposedRange range: NSRange, actualRange: NSRangePointer?) -> NSAttributedString? {
        guard imeCursor.height > 0, !chromeTextFocused, let wsHandle, range.location != NSNotFound else { return nil }
        let (end, overflow) = range.location.addingReportingOverflow(range.length)
        let span = CTextRange(
            none: false,
            start: CTextPosition(none: false, pos: UInt(range.location)),
            end: CTextPosition(none: false, pos: UInt(overflow ? Int.max : end)))
        guard let result = text_in_range(wsHandle, span) else { return nil }
        let text = String(cString: result)
        free_text(result)
        actualRange?.pointee = NSRange(location: range.location, length: text.utf16.count)
        return NSAttributedString(string: text)
    }
    public func validAttributesForMarkedText() -> [NSAttributedString.Key] { [] }
    public func firstRect(forCharacterRange _: NSRange, actualRange _: NSRangePointer?) -> NSRect {
        guard let window else { return .zero }
        guard imeCursor.height > 0 else { return window.convertToScreen(frame) }
        let rect = NSRect(x: imeCursor.minX, y: frame.height - imeCursor.maxY, width: imeCursor.width, height: imeCursor.height)
        return window.convertToScreen(convert(rect, to: nil))
    }
    public func characterIndex(for _: NSPoint) -> Int { NSNotFound }
}

extension NSPasteboard.PasteboardType {
    static let gif = Self(UTType.gif.identifier)
    static let webArchive = Self(UTType.webArchive.identifier)
}

/// The original file of an image a browser copied, which its web archive carries.
private func webArchiveImage(_ pasteBoard: NSPasteboard) -> Data? {
    guard let data = pasteBoard.data(forType: .webArchive),
          let archive = try? PropertyListSerialization.propertyList(from: data, format: nil) as? [String: Any]
    else { return nil }
    // the main resource names the image but holds an <img> tag; the bytes are a subresource
    let subresources = archive["WebSubresources"] as? [[String: Any]] ?? []
    return subresources
        .first { ($0["WebResourceMIMEType"] as? String)?.hasPrefix("image/") ?? false }?["WebResourceData"] as? Data
}
#endif
