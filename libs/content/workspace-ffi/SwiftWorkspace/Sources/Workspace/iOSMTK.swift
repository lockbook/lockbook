#if os(iOS)
    import Bridge
    import GameController
    import MetalKit
    import MobileCoreServices
    import SwiftUI
    import UIKit
    import UniformTypeIdentifiers

    // MARK: - SvgView

    public class SvgView: UIView {
        let mtkView: iOSMTK
        var wsHandle: UnsafeMutableRawPointer? {
            mtkView.wsHandle
        }

        /// pointer
        var pointerInteraction: UIPointerInteraction?

        // pencil
        var pencilDelegate: SvgPencilDelegate?
        var pencilInteraction: UIPencilInteraction?

        // gestures
        var gestureDelegate: SvgGestureDelegate?
        var tapRecognizer: UITapGestureRecognizer?
        var panRecognizer: UIPanGestureRecognizer?
        var pinchRecognizer: UIPinchGestureRecognizer?
        var pinchStartTouches = [(Float, Float)]()
        var pinchCumulativeScale: CGFloat = 1.0

        // menu
        var menuDelegate: SvgMenuDelegate?
        var menuInteraction: UIEditMenuInteraction?
        var editMenuForImage = false

        init(mtkView: iOSMTK) {
            self.mtkView = mtkView

            super.init(frame: .infinite)

            isMultipleTouchEnabled = true

            // pointer
            let pointerInteraction = UIPointerInteraction(delegate: mtkView.pointerDelegate)

            addInteraction(pointerInteraction)

            self.pointerInteraction = pointerInteraction

            // pencil
            let pencilDelegate = SvgPencilDelegate(mtkView: mtkView)
            let pencilInteraction = UIPencilInteraction()

            pencilInteraction.delegate = pencilDelegate
            addInteraction(pencilInteraction)

            self.pencilDelegate = pencilDelegate
            self.pencilInteraction = pencilInteraction

            // gestures
            gestureDelegate = SvgGestureDelegate()

            // gestures: tap
            let tap = UITapGestureRecognizer(target: self, action: #selector(handleTap(_:)))
            tap.allowedTouchTypes = [
                NSNumber(value: UITouch.TouchType.direct.rawValue),
                NSNumber(value: UITouch.TouchType.indirect.rawValue),
                // NSNumber(value: UITouch.TouchType.pencil.rawValue),
                NSNumber(value: UITouch.TouchType.indirectPointer.rawValue),
            ]
            tap.numberOfTouchesRequired = 1
            tap.cancelsTouchesInView = false

            addGestureRecognizer(tap)

            tapRecognizer = tap

            // gestures: pan
            let pan = UIPanGestureRecognizer(target: self, action: #selector(handlePan(_:)))
            pan.allowedTouchTypes = [
                NSNumber(value: UITouch.TouchType.direct.rawValue),
                NSNumber(value: UITouch.TouchType.indirect.rawValue),
                NSNumber(value: UITouch.TouchType.indirectPointer.rawValue),
            ]
            pan.allowedScrollTypesMask = .all
            pan.cancelsTouchesInView = false
            pan.delegate = gestureDelegate
            addGestureRecognizer(pan)
            panRecognizer = pan

            refreshPanTouchRequirementsFromCurrentTab()

            NotificationCenter.default.addObserver(
                self,
                selector: #selector(refreshPanTouchRequirementsFromCurrentTab),
                name: UIApplication.didBecomeActiveNotification,
                object: nil
            )

            // gestures: pinch
            let pinch = UIPinchGestureRecognizer(
                target: self, action: #selector(handlePinch(_:))
            )
            pinch.cancelsTouchesInView = false

            pinch.delegate = gestureDelegate
            addGestureRecognizer(pinch)

            pinchRecognizer = pinch

            // menu
            let menuDelegate = SvgMenuDelegate()
            let menuInteraction = UIEditMenuInteraction(delegate: menuDelegate)

            addInteraction(menuInteraction)

            menuDelegate.view = self
            self.menuDelegate = menuDelegate
            self.menuInteraction = menuInteraction
        }

        @objc private func refreshPanTouchRequirementsFromCurrentTab() {
            let currentTab = WorkspaceTab(rawValue: Int(current_tab(wsHandle)))
            applyPanTouchRequirements(for: currentTab)
        }

        func applyPanTouchRequirements(for currentTab: WorkspaceTab?) {
            let isImage = currentTab == .Image
            self.panRecognizer?.minimumNumberOfTouches =
                (isImage || UIPencilInteraction.prefersPencilOnlyDrawing) ? 1 : 2

            set_pencil_only_drawing(wsHandle, UIPencilInteraction.prefersPencilOnlyDrawing)
        }

        @objc private func handleTap(_ gesture: UITapGestureRecognizer) {
            guard let menuInteraction else { return }

            if gesture.state != .ended { return }

            if mtkView.kineticTimer != nil {
                mtkView.kineticTimer?.invalidate()
                mtkView.kineticTimer = nil
                return
            }

            if isImageTab {
                return
            }

            // Check if we have any valid actions before presenting
            let location = gesture.location(in: mtkView)
            if will_consume_touch(wsHandle, Float(location.x), Float(location.y))
                || (!UIPasteboard.general.hasStrings && !UIPasteboard.general.hasImages)
            {
                return
            }

            editMenuForImage = false
            let config = UIEditMenuConfiguration(identifier: nil, sourcePoint: location)
            menuInteraction.presentEditMenu(with: config)
        }

        var isImageTab: Bool {
            WorkspaceTab(rawValue: Int(current_tab(wsHandle))) == .Image
        }

        func presentImageEditMenu(at point: CGPoint) {
            guard let menuInteraction else { return }

            editMenuForImage = true
            let config = UIEditMenuConfiguration(identifier: nil, sourcePoint: point)
            menuInteraction.presentEditMenu(with: config)
        }

        override public func canPerformAction(_ action: Selector, withSender _: Any?) -> Bool {
            if action == #selector(paste(_:)) {
                return !isImageTab
                    && (UIPasteboard.general.hasStrings || UIPasteboard.general.hasImages)
            }

            return false
        }

        @objc func handlePan(_ sender: UIPanGestureRecognizer? = nil) {
            mtkView.handlePan(sender)
        }

        @objc func handlePinch(_ sender: UIPinchGestureRecognizer? = nil) {
            guard let event = sender, event.state != .cancelled, event.state != .failed else {
                return
            }

            if mtkView.kineticTimer != nil {
                mtkView.kineticTimer?.invalidate()
                mtkView.kineticTimer = nil
            }

            if event.state == .began {
                pinchStartTouches.removeAll()
                for i in 0..<(sender?.numberOfTouches ?? 0) {
                    let point = sender?.location(ofTouch: i, in: self)
                    if let p = point {
                        pinchStartTouches.append((Float(p.x), Float(p.y)))
                    }
                }
                pinchCumulativeScale = event.scale
            }

            let pinchCenter = event.location(in: mtkView)

            if event.state == .changed {
                let zoomDelta = Float(event.scale / pinchCumulativeScale)
                pinchCumulativeScale = event.scale

                multi_touch(
                    wsHandle,
                    0.0,
                    0.0,
                    zoomDelta,
                    Float(pinchCenter.x),
                    Float(pinchCenter.y),
                    pinchStartTouches.map{$0.0},
                    pinchStartTouches.map{$0.1},
                    UInt(pinchStartTouches.count)
                )
                mtkView.setNeedsDisplay()
            }
        }

        override public func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent?) {
            // let's cancel the kinetic pan. don't nullify it so that the tap handler
            // will know not to show the edit menu
            if mtkView.kineticTimer != nil {
                mtkView.kineticTimer?.invalidate()
            }

            mtkView.touchesBegan(touches, with: event)
        }

        override public func touchesMoved(_ touches: Set<UITouch>, with event: UIEvent?) {
            mtkView.touchesMoved(touches, with: event)
        }

        override public func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent?) {
            mtkView.touchesEnded(touches, with: event)
        }

        override public func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent?) {
            mtkView.touchesCancelled(touches, with: event)
        }

        override public func gestureRecognizerShouldBegin(
            _ gestureRecognizer: UIGestureRecognizer
        ) -> Bool {
            if isInteractiveContentPop(gestureRecognizer) {
                return false
            }
            return super.gestureRecognizerShouldBegin(gestureRecognizer)
        }

        override public func paste(_: Any?) {
            if isImageTab {
                return
            }

            if let image = UIPasteboard.general.image {
                mtkView.importContent(.image(image), isPaste: true)
            } else if let string = UIPasteboard.general.string {
                mtkView.importContent(.text(string), isPaste: true)
            }
        }

        @available(*, unavailable)
        required init(coder _: NSCoder) {
            fatalError("init(coder:) has not been implemented")
        }
    }

    // MARK: - SvgGestureDelegate

    public class SvgGestureDelegate: NSObject, UIGestureRecognizerDelegate {
        public func gestureRecognizer(
            _ gestureRecognizer: UIGestureRecognizer,
            shouldRecognizeSimultaneouslyWith otherGestureRecognizer: UIGestureRecognizer
        ) -> Bool {
            // Allow pinch and pan to work together
            if (gestureRecognizer is UIPinchGestureRecognizer
                && otherGestureRecognizer is UIPanGestureRecognizer)
                || (gestureRecognizer is UIPanGestureRecognizer
                    && otherGestureRecognizer is UIPinchGestureRecognizer)
            {
                return true
            }

            return false
        }
    }

    // MARK: - SvgPencilDelegate

    public class SvgPencilDelegate: NSObject, UIPencilInteractionDelegate {
        weak var mtkView: iOSMTK?

        init(mtkView: iOSMTK) {
            self.mtkView = mtkView
        }

        @available(iOS 17.5, *)
        public func pencilInteraction(
            _: UIPencilInteraction,
            didReceiveSqueeze squeeze: UIPencilInteraction.Squeeze
        ) {
            guard let mtkView else { return }

            if squeeze.phase == .ended {
                show_tool_popover_at_cursor(mtkView.wsHandle)
            }
        }

        public func pencilInteractionDidTap(_: UIPencilInteraction) {
            guard let mtkView else { return }

            switch UIPencilInteraction.preferredTapAction {
            case .ignore, .showColorPalette, .showInkAttributes:
                print("do nothing")
            case .switchEraser:
                toggle_drawing_tool_between_eraser(mtkView.wsHandle)
            case .switchPrevious:
                toggle_drawing_tool(mtkView.wsHandle)
            default:
                print("don't know, do nothing")
            }

            mtkView.setNeedsDisplay(mtkView.frame)
        }
    }

    // MARK: - SvgEditMenuDelegate

    public class SvgMenuDelegate: NSObject, UIEditMenuInteractionDelegate {
        weak var view: SvgView?

        public func editMenuInteraction(
            _: UIEditMenuInteraction, menuFor _: UIEditMenuConfiguration,
            suggestedActions: [UIMenuElement]
        ) -> UIMenu? {
            guard let view, view.editMenuForImage else {
                return UIMenu(children: suggestedActions)
            }

            let copy = UIAction(
                title: "Copy image", image: UIImage(systemName: "doc.on.doc")
            ) { [weak view] _ in
                guard let view else { return }
                copy_image(view.wsHandle)
                view.mtkView.setNeedsDisplay(view.mtkView.frame)
            }
            return UIMenu(children: [copy])
        }
    }

    // MARK: - iOSMTKViewDelegate

    public class iOSMTKViewDelegate: NSObject, MTKViewDelegate {
        weak var mtkView: iOSMTK?

        init(mtkView: iOSMTK) {
            self.mtkView = mtkView
        }

        public func mtkView(_: MTKView, drawableSizeWillChange size: CGSize) {
            guard let mtkView else { return }
            let wsHandle = mtkView.wsHandle
            resize_editor(
                wsHandle, Float(max(size.width, 1)), Float(max(size.height, 1)), Float(scale())
            )
            mtkView.setNeedsDisplay()
        }

        public func draw(in _: MTKView) {
            guard let mtkView else { return }
            let wsHandle = mtkView.wsHandle

            if mtkView.tabSwitchTask != nil {
                mtkView.tabSwitchTask!()
                mtkView.tabSwitchTask = nil
            }

            dark_mode(wsHandle, mtkView.isDarkMode())
            syncAccentColor(mtkView)
            set_contact_linked_sites(wsHandle, UserDefaults.standard.bool(forKey: "contactLinkedSites"))
            set_open_in_new_tab(wsHandle, UserDefaults.standard.object(forKey: "openInNewTab") as? Bool ?? true)

            set_scale(wsHandle, Float(scale()))
            let keyboardTop = mtkView.keyboardLayoutGuide.layoutFrame.minY
            let overlap = max(0, mtkView.bounds.maxY - keyboardTop)
            set_ws_inset(wsHandle, Float(overlap * scale()))

            handle(ios_frame(wsHandle))
            VoiceEngine.shared.service(wsHandle)
        }

        /// A frame that consumes no queued input, so geometry reflects an edit
        /// UIKit just made before UIKit asks for it.
        func layoutFrame() {
            guard let mtkView else { return }
            handle(ios_layout_frame(mtkView.wsHandle))
        }

        private func handle(_ output: IOSResponse) {
            guard let mtkView else { return }
            let wsHandle = mtkView.wsHandle

            if output.tabs_changed {
                mtkView.workspaceOutput?.tabCount = Int(tab_count(wsHandle))
            }

            if output.open_camera {
                mtkView.workspaceOutput?.openCamera = true
            }

            if output.mobile_toolbar_shown != mtkView.workspaceOutput?.mobileToolbarShown {
                mtkView.workspaceOutput?.mobileToolbarShown = output.mobile_toolbar_shown
            }

            if output.selected_folder_changed {
                let selectedFolder = UUID(uuid: get_selected_folder(wsHandle)._0)
                if selectedFolder.isNil() {
                    mtkView.workspaceOutput?.selectedFolder = nil
                } else {
                    mtkView.workspaceOutput?.selectedFolder = selectedFolder
                }
            }

            let selectedFile = UUID(uuid: output.selected_file._0)
            let selectedSession = UUID(uuid: output.selected_tab._0)
            // selected_file / selected_tab are one-frame events.
            if !selectedSession.isNil() {
                mtkView.workspaceOutput?.currentSession = selectedSession
            }
            if !selectedFile.isNil() {
                if mtkView.currentOpenDoc != selectedFile {
                    (mtkView.currentWrapper as? TextPage)?.documentReplaced()
                }

                mtkView.currentOpenDoc = selectedFile

                if selectedFile != mtkView.workspaceOutput?.openDoc {
                    mtkView.workspaceOutput?.openDoc = selectedFile
                }
            }

            let currentTab = WorkspaceTab(rawValue: Int(current_tab(wsHandle))) ?? .Welcome
            if currentTab == .Welcome {
                mtkView.workspaceOutput?.currentSession = nil
            }
            if currentTab == .Welcome || currentTab == .Search, mtkView.currentOpenDoc != nil {
                mtkView.currentOpenDoc = nil
                mtkView.workspaceOutput?.openDoc = nil
            }

            if currentTab != mtkView.workspaceOutput!.currentTab {
                if let currentWrapper = mtkView.currentWrapper as? SvgView {
                    currentWrapper.applyPanTouchRequirements(for: currentTab)
                }

                DispatchQueue.main.async {
                    mtkView.workspaceOutput!.currentTab = currentTab
                    mtkView.currentTabChanged?(currentTab)
                }
            }

            if output.has_context_menu, output.context_menu_for_image,
               let currentWrapper = mtkView.currentWrapper as? SvgView
            {
                currentWrapper.presentImageEditMenu(
                    at: CGPoint(
                        x: CGFloat(output.context_menu_x), y: CGFloat(output.context_menu_y)
                    )
                )
            }

            let created = UUID(uuid: output.doc_created._0)
            if !created.isNil() {
                mtkView.workspaceInput?.pendingFocusDoc = created
            }

            if let text = mtkView.currentWrapper as? TextPage,
               currentTab == .Markdown || currentTab == .PlainText || currentTab == .Chat
            {
                text.apply(output)
                if let pending = mtkView.workspaceInput?.pendingFocusDoc, pending == mtkView.currentOpenDoc {
                    mtkView.workspaceInput?.pendingFocusDoc = nil
                    text.becomeFirstResponder()
                }
            }

            if output.urls_opened.size > 0 {
                var urls: [URL] = []
                for i in 0..<Int(output.urls_opened.size) {
                    // Don't use textFromPtr here — it frees each string, but
                    // free_urls below frees the strings and the array together.
                    if let ptr = output.urls_opened.urls[i],
                       let url = URL(string: String(cString: ptr)),
                       UIApplication.shared.canOpenURL(url)
                    {
                        urls.append(url)
                    }
                }
                mtkView.workspaceOutput?.urlsOpened = urls
                free_urls(output.urls_opened)
            }

            if let text = output.copied_text {
                let text = textFromPtr(s: text)
                if !text.isEmpty {
                    UIPasteboard.general.string = text
                }
            }

            if let png = dataFromBytes(b: output.copied_image) {
                UIPasteboard.general.setData(png, forPasteboardType: UTType.png.identifier)
            }

            mtkView.redrawTask?.cancel()
            mtkView.redrawTask = nil
            mtkView.redrawDeadline = nil
            mtkView.isPaused = output.redraw_in > 50
            if mtkView.isPaused, output.redraw_in != UInt64.max {
                mtkView.scheduleRedraw(inMs: UInt64(truncatingIfNeeded: output.redraw_in))
            }

            mtkView.enableSetNeedsDisplay = mtkView.isPaused
        }

        private var lastAccentColors: UInt64 = 0

        private func syncAccentColor(_ mtkView: iOSMTK) {
            let light = packedAccent(
                mtkView.tintColor.resolvedColor(with: UITraitCollection(userInterfaceStyle: .light))
            )
            let dark = packedAccent(
                mtkView.tintColor.resolvedColor(with: UITraitCollection(userInterfaceStyle: .dark))
            )

            let combined = (UInt64(light) << 32) | UInt64(dark)
            guard combined != lastAccentColors else { return }
            lastAccentColors = combined

            set_accent_color(mtkView.wsHandle, light, dark)
        }

        private func packedAccent(_ color: UIColor) -> UInt32 {
            var r: CGFloat = 0
            var g: CGFloat = 0
            var b: CGFloat = 0
            var a: CGFloat = 0
            color.getRed(&r, green: &g, blue: &b, alpha: &a)

            let ri = UInt32((min(max(r, 0), 1) * 255).rounded())
            let gi = UInt32((min(max(g, 0), 1) * 255).rounded())
            let bi = UInt32((min(max(b, 0), 1) * 255).rounded())
            return (ri << 24) | (gi << 16) | (bi << 8) | 0xFF
        }

        func scale() -> CGFloat {
            mtkView?.contentScaleFactor ?? CGFloat(1.0)
        }
    }

    // MARK: - iOSPointerDelegate

    public class iOSPointerDelegate: NSObject, UIPointerInteractionDelegate {
        weak var mtkView: iOSMTK?

        init(mtkView: iOSMTK) {
            self.mtkView = mtkView
        }

        public func pointerInteraction(
            _ interaction: UIPointerInteraction, regionFor request: UIPointerRegionRequest,
            defaultRegion: UIPointerRegion
        ) -> UIPointerRegion? {
            guard let mtkView else { return defaultRegion }
            let wsHandle = mtkView.wsHandle

            let location = interaction.view?.convert(request.location, to: mtkView)
                ?? request.location
            mouse_moved(wsHandle, Float(location.x), Float(location.y))
            return defaultRegion
        }

        public func pointerInteraction(
            _: UIPointerInteraction, willEnter _: UIPointerRegion,
            animator _: any UIPointerInteractionAnimating
        ) {
            guard let mtkView else { return }

            mtkView.cursorTracked = true
        }

        public func pointerInteraction(
            _: UIPointerInteraction, willExit _: UIPointerRegion,
            animator _: any UIPointerInteractionAnimating
        ) {
            guard let mtkView else { return }

            mtkView.cursorTracked = false
            mouse_gone(mtkView.wsHandle)
        }
    }

    // MARK: - iOSMTK

    public class iOSMTK: MTKView {
        public static let POINTER_DECELERATION_RATE: CGFloat = 0.95

        public var wsHandle: UnsafeMutableRawPointer?
        var claimedPersistence = false
        weak var currentWrapper: UIView?

        // pointer
        var pointerInteraction: UIPointerInteraction?
        var pointerDelegate: UIPointerInteractionDelegate?

        // touch ids
        private var touchMap = [ObjectIdentifier: UInt64]()
        private var nextTouchID: UInt64 = 0

        /// gestures
        var panRecognizer: UIPanGestureRecognizer?
        var panStartTouches = [(Float, Float)]()


        // mtk
        var mtkDelegate: iOSMTKViewDelegate?
        var redrawTask: DispatchWorkItem?
        var redrawDeadline: DispatchTime?

        // workspace
        var workspaceOutput: WorkspaceOutputState?
        var workspaceInput: WorkspaceInputState?
        var currentTabChanged: ((WorkspaceTab) -> Void)?
        var currentOpenDoc: UUID? // TODO: duplicated in ws output

        // view hierarchy management
        var tabSwitchTask: (() -> Void)? // facilitates switching wrapper views in response to tab change

        // kinetic scroll
        var cursorTracked = false
        var scrollSensitivity = 50.0
        var scrollId = 0
        var kineticTimer: Timer?

        override init(frame frameRect: CGRect, device: MTLDevice?) {
            super.init(frame: frameRect, device: device)

            // pointer
            let pointerDelegate = iOSPointerDelegate(mtkView: self)
            let pointer = UIPointerInteraction(delegate: pointerDelegate)

            addInteraction(pointer)

            self.pointerDelegate = pointerDelegate
            pointerInteraction = pointer

            // gestures
            let pan = UIPanGestureRecognizer(
                target: self, action: #selector(handleTrackpadScroll(_:))
            )
            pan.allowedScrollTypesMask = .all
            pan.maximumNumberOfTouches = 0

            addGestureRecognizer(pan)
            panRecognizer = pan

            // mtk
            mtkDelegate = iOSMTKViewDelegate(mtkView: self)

            isPaused = false
            enableSetNeedsDisplay = false
            delegate = mtkDelegate
            // A hidden keyboard insets nothing; the page runs to the screen edge.
            keyboardLayoutGuide.usesBottomSafeArea = false
            preferredFramesPerSecond = 144
            isUserInteractionEnabled = true

            NotificationCenter.default.addObserver(
                self,
                selector: #selector(appDidBecomeActive),
                name: UIApplication.didBecomeActiveNotification,
                object: nil
            )
            NotificationCenter.default.addObserver(
                self,
                selector: #selector(appDidEnterBackground),
                name: UIApplication.didEnterBackgroundNotification,
                object: nil
            )
        }

        @objc private func appDidBecomeActive() {
            self.isPaused = false
        }

        @objc private func appDidEnterBackground() {
            self.isPaused = true
        }

        required init(coder: NSCoder) {
            fatalError("init(coder:) has not been implemented")
        }

        @objc func handleTrackpadScroll(_ sender: UIPanGestureRecognizer? = nil) {
            guard let event = sender, event.state != .cancelled, event.state != .failed else {
                return
            }

            var velocity = event.velocity(in: self)

            velocity.x /= 50
            velocity.y /= 50

            if event.state == .ended {
                let currentScrollId = Int(Date().timeIntervalSince1970)
                scrollId = currentScrollId

                Timer.scheduledTimer(withTimeInterval: 0.016, repeats: true) { [self] timer in
                    if currentScrollId != scrollId {
                        timer.invalidate()
                        return
                    }

                    velocity.x *= Self.POINTER_DECELERATION_RATE
                    velocity.y *= Self.POINTER_DECELERATION_RATE

                    if abs(velocity.x) < 0.1, abs(velocity.y) < 0.1 {
                        timer.invalidate()
                        return
                    }

                    if !cursorTracked {
                        mouse_moved(wsHandle, Float(bounds.width / 2), Float(bounds.height / 2))
                    }
                    scroll_wheel(
                        wsHandle, Float(velocity.x), Float(velocity.y), false, false, false,
                        false
                    )
                    if !cursorTracked {
                        mouse_gone(wsHandle)
                    }

                    setNeedsDisplay()
                }
            } else {
                if !cursorTracked {
                    mouse_moved(wsHandle, Float(bounds.width / 2), Float(bounds.height / 2))
                }
                scroll_wheel(
                    wsHandle, Float(velocity.x), Float(velocity.y), false, false, false, false
                )
                if !cursorTracked {
                    mouse_gone(wsHandle)
                }
            }

            setNeedsDisplay()
        }

        /// used in canvas
        @objc func handlePan(_ sender: UIPanGestureRecognizer? = nil) {
            guard let event = sender, event.state != .cancelled, event.state != .failed else {
                return
            }

            if event.state == .began {
                panStartTouches.removeAll()
                kineticTimer?.invalidate()
                kineticTimer = nil
                for i in 0..<(sender?.numberOfTouches ?? 0){
                    let point = sender?.location(ofTouch: i, in: self)
                    if let p = point {
                        panStartTouches.append((Float(p.x), Float(p.y)))
                    }
                }
            }

            var velocity = event.velocity(in: self)

            velocity.x /= scrollSensitivity
            velocity.y /= scrollSensitivity

            if event.state == .ended {
                let currentScrollId = Int(Date().timeIntervalSince1970)
                scrollId = currentScrollId
                touches_ended(wsHandle, UInt64.random(in: UInt64.min ... UInt64.max), 0.0, 0.0, 0.0)

                kineticTimer = Timer.scheduledTimer(withTimeInterval: 0.016, repeats: true) {
                    [weak self] timer in
                    guard let self else {
                        timer.invalidate()
                        self?.kineticTimer = nil
                        return
                    }

                    velocity.x *= Self.POINTER_DECELERATION_RATE
                    velocity.y *= Self.POINTER_DECELERATION_RATE

                    if abs(velocity.x) < 0.1, abs(velocity.y) < 0.1 {
                        timer.invalidate()
                        kineticTimer = nil
                        return
                    }

                    multi_touch(
                        wsHandle,
                        Float(velocity.x),
                        Float(velocity.y),
                        1.0,
                        0.0,
                        0.0,
                        panStartTouches.map{$0.0},
                        panStartTouches.map{$0.1},
                        UInt(panStartTouches.count)
                    )


                    self.setNeedsDisplay()
                }
            } else {
                let translation = event.translation(in: self)


                multi_touch(
                    wsHandle,
                    Float(translation.x),
                    Float(translation.y),
                    1.0,
                    0.0,
                    0.0,
                    panStartTouches.map{$0.0},
                    panStartTouches.map{$0.1},
                    UInt(panStartTouches.count)
                )

                event.setTranslation(.zero, in: self)
            }

            setNeedsDisplay()
        }

        override public func didMoveToWindow() {
            super.didMoveToWindow()

            if let screen = window?.screen {
                preferredFramesPerSecond = screen.maximumFramesPerSecond
            }
        }

        public func setInitialContent(_ coreHandle: UnsafeMutableRawPointer?) {
            let metalLayer = UnsafeMutableRawPointer(
                Unmanaged.passUnretained(layer).toOpaque()
            )
            claimedPersistence = true
            wsHandle = init_ws(coreHandle, metalLayer, isDarkMode(), false, WorkspacePersistence.claim())
            workspaceInput?.wsHandle = wsHandle

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
                self?.drawImmediately()
            }
            redrawTask = task
            redrawDeadline = deadline
            DispatchQueue.main.asyncAfter(deadline: deadline, execute: task)
        }

        /// Draw on the next display refresh, even if the loop is paused.
        func requestFrame() {
            isPaused = false
            enableSetNeedsDisplay = false
        }

        /// Lay out and present now, without consuming queued input.
        func layoutFrame() {
            redrawTask?.cancel()
            redrawTask = nil
            redrawDeadline = nil
            mtkDelegate?.layoutFrame()
        }

        public func drawImmediately() {
            redrawTask?.cancel()
            redrawTask = nil
            redrawDeadline = nil

            isPaused = true
            enableSetNeedsDisplay = false

            mtkDelegate?.draw(in: self)
        }

        override public func traitCollectionDidChange(_: UITraitCollection?) {
            setNeedsDisplay(frame)
        }

        override public func gestureRecognizerShouldBegin(
            _ gestureRecognizer: UIGestureRecognizer
        ) -> Bool {
            if isInteractiveContentPop(gestureRecognizer) {
                return false
            }
            return super.gestureRecognizerShouldBegin(gestureRecognizer)
        }

        override public func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent?) {
            for touch in touches {
                let value = getTouchID(for: touch, createIfMissing: true)

                for touch in event!.coalescedTouches(for: touch)! {
                    let location = touch.preciseLocation(in: self)
                    let force = touch.force != 0 ? touch.force / touch.maximumPossibleForce : 0
                    touches_began(
                        wsHandle, value, Float(location.x), Float(location.y), Float(force)
                    )
                }
            }

            setNeedsDisplay(frame)
        }

        private func getTouchID(for touch: UITouch, createIfMissing: Bool = false) -> UInt64 {
            let key = ObjectIdentifier(touch)
            if let existingID = touchMap[key] {
                return existingID
            }
            if createIfMissing {
                let newID = nextTouchID
                touchMap[key] = newID
                nextTouchID += 1
                return newID
            }
            return 0
        }

        override public func touchesMoved(_ touches: Set<UITouch>, with event: UIEvent?) {
            for touch in touches {
                let value = getTouchID(for: touch)

                let location = touch.preciseLocation(in: self)
                let force = touch.force != 0 ? touch.force / touch.maximumPossibleForce : 0

                for touch in event!.predictedTouches(for: touch)! {
                    let location = touch.preciseLocation(in: self)
                    let force = touch.force != 0 ? touch.force / touch.maximumPossibleForce : 0
                    touches_predicted(
                        wsHandle, value, Float(location.x), Float(location.y), Float(force)
                    )
                }

                touches_moved(wsHandle, value, Float(location.x), Float(location.y), Float(force))
            }

            setNeedsDisplay(frame)
        }

        override public func touchesEnded(_ touches: Set<UITouch>, with _: UIEvent?) {
            for touch in touches {
                let value = getTouchID(for: touch)

                let location = touch.preciseLocation(in: self)
                let force = touch.force != 0 ? touch.force / touch.maximumPossibleForce : 0
                touches_ended(wsHandle, value, Float(location.x), Float(location.y), Float(force))

                touchMap.removeValue(forKey: ObjectIdentifier(touch))
            }

            setNeedsDisplay(frame)
        }

        override public func touchesCancelled(_ touches: Set<UITouch>, with _: UIEvent?) {
            for touch in touches {
                let value = getTouchID(for: touch)

                let location = touch.preciseLocation(in: self)
                let force = touch.force != 0 ? touch.force / touch.maximumPossibleForce : 0

                touches_cancelled(
                    wsHandle, value, Float(location.x), Float(location.y), Float(force)
                )
            }

            setNeedsDisplay(frame)
        }

        override public func pressesBegan(_ presses: Set<UIPress>, with event: UIPressesEvent?) {
            let forward = handleKeyEvent(presses, with: event, pressBegan: true)
            if forward {
                super.pressesBegan(presses, with: event)
            }
        }

        override public func pressesEnded(_ presses: Set<UIPress>, with event: UIPressesEvent?) {
            let forward = handleKeyEvent(presses, with: event, pressBegan: false)
            if forward {
                super.pressesEnded(presses, with: event)
            }
        }

        /// Returns whether the event should be forwarded up the inheritance hierarchy
        func handleKeyEvent(_ presses: Set<UIPress>, with _: UIPressesEvent?, pressBegan: Bool)
            -> Bool
        {
            var forward = true

            for press in presses {
                guard let key = press.key else { continue }

                if workspaceOutput!.currentTab.isTextEdit()
                    && key.keyCode == .keyboardDeleteOrBackspace
                {
                    break
                }

                var shift = key.modifierFlags.contains(.shift)
                var ctrl = key.modifierFlags.contains(.control)
                var option = key.modifierFlags.contains(.alternate)
                var command = key.modifierFlags.contains(.command)

                // On a modifier key's own key-up, modifierFlags still reports
                // it as held, so egui's persistent modifier state stays stuck
                // (a later tap then reads cmd as down and opens links). Clear
                // the released modifier.
                if !pressBegan {
                    switch key.keyCode {
                    case .keyboardLeftGUI, .keyboardRightGUI: command = false
                    case .keyboardLeftShift, .keyboardRightShift: shift = false
                    case .keyboardLeftControl, .keyboardRightControl: ctrl = false
                    case .keyboardLeftAlt, .keyboardRightAlt: option = false
                    default: break
                    }
                }

                if (command && key.keyCode == .keyboardW) || (shift && key.keyCode == .keyboardTab) {
                    forward = false
                }

                ios_key_event(
                    wsHandle, key.keyCode.rawValue, shift, ctrl, option, command, pressBegan
                )
                setNeedsDisplay(frame)
            }

            return forward
        }

        override public var keyCommands: [UIKeyCommand]? {
            iOSMTK.workspaceBracketKeyCommands()
        }

        @objc func forwardBracketCommand(_ command: UIKeyCommand) {
            guard let input = command.input else { return }
            let keyCode: Int
            switch input {
            case "[": keyCode = UIKeyboardHIDUsage.keyboardOpenBracket.rawValue
            case "]": keyCode = UIKeyboardHIDUsage.keyboardCloseBracket.rawValue
            default: return
            }

            let mods = command.modifierFlags

            ios_key_event(
                wsHandle, keyCode, mods.contains(.shift), mods.contains(.control),
                mods.contains(.alternate), mods.contains(.command), true
            )
            // Release with no modifiers: this combo is swallowed by UIKeyCommand,
            // so the physical cmd key-up never reaches handleKeyEvent to clear it.
            ios_key_event(wsHandle, keyCode, false, false, false, false, false)
            setNeedsDisplay(frame)
        }

        static func workspaceBracketKeyCommands() -> [UIKeyCommand] {
            var commands: [UIKeyCommand] = []
            let modifierSets: [UIKeyModifierFlags] = [
                [.command, .shift], [.command, .control, .shift],
            ]
            for input in ["[", "]"] {
                for modifierFlags in modifierSets {
                    let command = UIKeyCommand(
                        input: input, modifierFlags: modifierFlags,
                        action: #selector(iOSMTK.forwardBracketCommand(_:))
                    )
                    command.wantsPriorityOverSystemBehavior = true
                    commands.append(command)
                }
            }
            return commands
        }

        func importContent(_ importFormat: SupportedImportFormat, isPaste: Bool) {
            switch importFormat {
            case let .url(url):
                if url.pathExtension.lowercased() == "png" {
                    guard let data = try? Data(contentsOf: url) else {
                        return
                    }

                    workspaceInput?.pasteImage(data: data, isPaste: isPaste)
                } else {
                    clipboard_send_file(wsHandle, url.path(percentEncoded: false), isPaste)
                }
            case let .image(image):
                let image = image.normalizedImage()
                if let data = image.pngData() ?? image.jpegData(compressionQuality: 1.0) {
                    workspaceInput?.pasteImage(data: data, isPaste: isPaste)
                }
            case let .text(text):
                clipboard_paste(wsHandle, text)
            }
        }

        func isDarkMode() -> Bool {
            traitCollection.userInterfaceStyle != .light
        }

        deinit {
            if let wsHandle {
                RepaintRelay.unregister(wsHandle)
                VoiceEngine.shared.release(wsHandle)
            }
            deinit_editor(wsHandle)

            if claimedPersistence {
                WorkspacePersistence.release()
            }
        }

        func unimplemented() {
            print("unimplemented!")
            Thread.callStackSymbols.forEach { print($0) }
            //        exit(-69)
        }

        override public var canBecomeFocused: Bool {
            true
        }

        override public var canBecomeFirstResponder: Bool {
            true
        }
    }

    public enum SupportedImportFormat {
        case url(URL)
        case image(UIImage)
        case text(String)
    }

    extension UIView {
        func isInteractiveContentPop(_ gestureRecognizer: UIGestureRecognizer) -> Bool {
            var responder = next
            while let current = responder {
                if let controller = current as? UIViewController {
                    return gestureRecognizer
                        === controller.navigationController?
                        .interactiveContentPopGestureRecognizer
                }
                responder = current.next
            }
            return false
        }
    }

#endif

public enum WorkspaceTab: Int {
    case Welcome = 0
    case Loading = 1
    case Image = 2
    case Markdown = 3
    case PlainText = 4
    case Pdf = 5
    case Svg = 6
    case Graph = 7
    case SpaceInspector = 8
    case Chat = 9
    case Search = 10

    func viewWrapperId() -> Int {
        switch self {
        case .Welcome, .Pdf, .Loading, .SpaceInspector, .Search:
            1
        case .Svg, .Image, .Graph:
            2
        case .PlainText, .Markdown, .Chat:
            3
        }
    }

    func isTextEdit() -> Bool {
        self == .Markdown || self == .PlainText
    }

    func isSvg() -> Bool {
        self == .Svg
    }
}
