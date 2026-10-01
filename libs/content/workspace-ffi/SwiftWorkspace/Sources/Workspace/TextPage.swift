#if os(iOS)
    import Bridge
    import UIKit

    /// The page of text: a tap layer, the text input view above it, and
    /// the recognizers that decide the page's touches. A touch on a touch target is
    /// hit-tested to the tap layer, so UIKit's text gestures never see it;
    /// the page's own pan and coast-stop see every touch on the page, so a
    /// drag from a touch target scrolls and a touch during a coast stops it.
    final class TextPage: UIView {
        unowned let mtkView: iOSMTK
        var wsHandle: UnsafeMutableRawPointer? { mtkView.wsHandle }

        /// A pending selection commits once no finger is on the page.
        let touches = TouchWatch()
        private(set) var text: TextInputView!
        private(set) var taps: TapLayer!
        let scrollPan = UIPanGestureRecognizer()
        let coastStop = CoastStopRecognizer()
        private var observedDrags: [UIGestureRecognizer] = []

        /// A UIKit handle or loupe drag is in progress or just ending.
        var textDragActive: Bool {
            observedDrags.contains { [.began, .changed, .ended].contains($0.state) }
        }

        /// A UIKit handle or loupe drag is under a finger still down.
        var textDragMoving: Bool {
            observedDrags.contains { [.began, .changed].contains($0.state) }
        }

        /// The page scrolled under a drag since the last frame.
        private var crawledSinceFrame = false
        let scrollbar = ScrollbarDragRecognizer()
        private(set) var crawl: EdgeCrawl!


        init(mtkView: iOSMTK) {
            self.mtkView = mtkView
            super.init(frame: .zero)
            backgroundColor = .clear
            clipsToBounds = true

            crawl = EdgeCrawl(page: self)
            taps = TapLayer(page: self)
            text = TextInputView(mtkView: mtkView, page: self)
            for view in [taps!, text!] as [UIView] {
                view.frame = bounds
                view.autoresizingMask = [.flexibleWidth, .flexibleHeight]
                addSubview(view)
            }

            scrollPan.addTarget(self, action: #selector(handleScrollPan(_:)))
            scrollPan.allowedScrollTypesMask = .all
            // A trackpad click-drag selects; its two-finger scroll still arrives.
            scrollPan.allowedTouchTypes = [UITouch.TouchType.direct, .pencil].map {
                NSNumber(value: $0.rawValue)
            }
            scrollPan.delegate = self
            addGestureRecognizer(scrollPan)

            coastStop.stopCoast = { [weak self] in
                guard let self, let wsHandle = self.wsHandle else { return false }
                return ios_stop_scroll(wsHandle)
            }
            coastStop.delegate = self
            addGestureRecognizer(coastStop)

            // The coast-stop decides at touch-down, so the tap never waits.
            taps.tap.require(toFail: coastStop)

            // The scrollbar takes its touch at touch-down; the page pan and
            // the layer's tap are then out of the running.
            scrollbar.isOnScrollbar = { [weak self] point in
                guard let self, let wsHandle = self.wsHandle, self.touchTarget(at: point) == 2 else {
                    return false
                }
                let p = self.convert(point, to: self.mtkView)
                return ios_on_scroll_thumb(wsHandle, Float(p.y))
            }
            scrollbar.begin = { [weak self] _ in
                guard let self, let wsHandle = self.wsHandle else { return }
                ios_scrollbar_begin(wsHandle)
                self.mtkView.requestFrame()
            }
            scrollbar.drag = { [weak self] dy in
                guard let self, let wsHandle = self.wsHandle else { return }
                ios_scrollbar_drag(wsHandle, Float(dy))
                self.mtkView.requestFrame()
            }
            scrollbar.delegate = self
            addGestureRecognizer(scrollbar)

            touches.delegate = self
            addGestureRecognizer(touches)
        }

        @available(*, unavailable)
        required init?(coder _: NSCoder) {
            fatalError("init(coder:) has not been implemented")
        }

        /// The page itself takes no touches: they belong to the text view, the
        /// tap layer, or whatever lies beneath (popups, the toolbar band).
        override func hitTest(_ point: CGPoint, with event: UIEvent?) -> UIView? {
            let hit = super.hitTest(point, with: event)
            return hit === self ? nil : hit
        }

        override var canBecomeFirstResponder: Bool { text.canBecomeFirstResponder }

        @discardableResult
        override func becomeFirstResponder() -> Bool {
            text.becomeFirstResponder()
        }

        @discardableResult
        override func resignFirstResponder() -> Bool {
            text.resignFirstResponder()
        }

        // MARK: - Touch targets

        /// What the editor painted under a point of the page, as
        /// `ios_touch_target_at` numbers it; 0 for text.
        func touchTarget(at point: CGPoint) -> UInt8 {
            guard let wsHandle else { return 0 }
            let p = convert(point, to: mtkView)
            return ios_touch_target_at(wsHandle, Float(p.x), Float(p.y))
        }

        /// Tap the target. A link or an image answers the range to select,
        /// which the text view does with its menu.
        func tap(at point: CGPoint) {
            guard let wsHandle else { return }
            let p = convert(point, to: mtkView)
            let select = ios_tap(wsHandle, Float(p.x), Float(p.y))
            mtkView.requestFrame()
            if !select.none {
                text.selectAtom(TextRange(Int(select.start.pos), Int(select.end.pos)))
            }
        }

        // MARK: - Frame output

        func apply(_ output: IOSResponse) {
            if output.has_text_interaction_rect {
                let r = output.text_interaction_rect
                let rect = CGRect(
                    x: r.min_x.rounded(), y: r.min_y.rounded(),
                    width: (r.max_x - r.min_x).rounded(), height: (r.max_y - r.min_y).rounded()
                )
                let local = superview.map { $0.convert(rect, from: mtkView) } ?? rect
                if frame != local {
                    frame = local
                }
            }
            text.apply(output)
            // The page moved under a still finger: the selection follows.
            if crawledSinceFrame, text.selectionMoved, textDragActive, crawl.point != nil {
                text.followCrawl()
            }
            crawledSinceFrame = false
        }

        func documentReplaced() {
            text.documentReplaced()
        }

        // MARK: - Page scroll

        /// Crawl the page by `delta` points of content travel: positive
        /// moves the content up, as a finger dragging up would. A crawl
        /// stops at the document's end.
        func crawl(by delta: CGFloat) {
            guard let wsHandle else { return }
            guard delta.isFinite else {
                return
            }
            ios_crawl(wsHandle, Float(-delta))
            crawledSinceFrame = true
            mtkView.requestFrame()
        }

        var rowHeight: CGFloat {
            wsHandle.map { CGFloat(ios_row_height($0)) } ?? 28
        }

        /// UIKit's handle drag and loupe drag, seen once they share a touch
        /// with the page's recognizers. A target only observes.
        fileprivate func observeTextDrag(_ recognizer: UIGestureRecognizer) {
            guard recognizer.isRangeAdjustment || recognizer.isCaretDrag || recognizer.isSelectionDrag,
                  !observedDrags.contains(where: { $0 === recognizer })
            else { return }
            observedDrags.append(recognizer)
            recognizer.addTarget(self, action: #selector(textDragChanged(_:)))
        }

        /// The finger near the edge during a handle or loupe drag crawls
        /// the page. A loupe crawls only once the finger has moved.
        @objc private func textDragChanged(_ recognizer: UIGestureRecognizer) {
            switch recognizer.state {
            case .began:
                text.dragBegan()
                if recognizer.isRangeAdjustment {
                    crawl.update(recognizer.location(in: self))
                }
            case .changed:
                crawl.update(recognizer.location(in: self))
            case .ended, .cancelled, .failed:
                crawl.stop()
            default:
                break
            }
        }

        @objc private func handleScrollPan(_ pan: UIPanGestureRecognizer) {
            guard let wsHandle else { return }
            // UIKit keeps its menu's fate to itself during its own drags, and
            // dismissing it from here cancels the drag.
            if pan.state == .began, !textDragActive {
                text.dismissEditMenus()
            }
            switch pan.state {
            case .began, .changed:
                let dy = pan.translation(in: self).y
                pan.setTranslation(.zero, in: self)
                ios_scroll(wsHandle, Float(dy))
                mtkView.requestFrame()
            case .ended:
                let dy = pan.translation(in: self).y
                pan.setTranslation(.zero, in: self)
                let vy = pan.velocity(in: self).y
                ios_scroll(wsHandle, Float(dy))
                ios_fling(wsHandle, Float(vy))
                mtkView.requestFrame()
            default:
                break
            }
        }

        override func gestureRecognizerShouldBegin(_ gestureRecognizer: UIGestureRecognizer) -> Bool {
            if isInteractiveContentPop(gestureRecognizer) {
                return false
            }
            return super.gestureRecognizerShouldBegin(gestureRecognizer)
        }
    }

    extension TextPage: UIGestureRecognizerDelegate {
        /// A handle drag wins over the page scroll. Off a handle, that gesture
        /// fails at touch-down, so the scroll does not wait.
        func gestureRecognizer(
            _ gestureRecognizer: UIGestureRecognizer,
            shouldRequireFailureOf otherGestureRecognizer: UIGestureRecognizer
        ) -> Bool {
            gestureRecognizer === scrollPan && otherGestureRecognizer.isRangeAdjustment
        }

        /// Stopping a coast keeps the finger for a scroll or a handle drag,
        /// and nothing else. A list-item hold runs beside the pan, which the
        /// editor ignores while the item is lifted. The touch watch sees all.
        func gestureRecognizer(
            _ gestureRecognizer: UIGestureRecognizer,
            shouldRecognizeSimultaneouslyWith otherGestureRecognizer: UIGestureRecognizer
        ) -> Bool {
            observeTextDrag(otherGestureRecognizer)
            if gestureRecognizer === touches || otherGestureRecognizer === touches {
                return true
            }
            if gestureRecognizer === coastStop {
                return otherGestureRecognizer === scrollPan || otherGestureRecognizer === scrollbar
                    || otherGestureRecognizer.isRangeAdjustment || otherGestureRecognizer.isSelectionDrag
            }
            if gestureRecognizer === scrollbar {
                return otherGestureRecognizer === coastStop
            }
            if gestureRecognizer === scrollPan {
                return otherGestureRecognizer is ReorderPressRecognizer
            }
            return false
        }
    }

    /// A selection handle, the loupe, or the spacebar cursor near the
    /// viewport's edge crawls the page. Speed grows linearly with depth:
    /// nothing one and a half rows inside the edge, thirty rows a second
    /// at the edge, and thirty more for every further band beyond the
    /// viewport, up to ninety. Top and bottom are alike.
    final class EdgeCrawl {
        private weak var page: TextPage?
        private var link: CADisplayLink?
        private var lastTick: CFTimeInterval?
        /// The finger's latest position while a crawl may run.
        private(set) var point: CGPoint?

        init(page: TextPage) {
            self.page = page
        }

        /// The finger's latest position in the page; it may lie outside.
        func update(_ point: CGPoint) {
            self.point = point
            if link == nil {
                let link = CADisplayLink(target: self, selector: #selector(tick(_:)))
                link.add(to: .main, forMode: .common)
                self.link = link
            }
        }

        func stop() {
            link?.invalidate()
            link = nil
            lastTick = nil
            point = nil
        }

        @objc private func tick(_ link: CADisplayLink) {
            guard let point, let page else {
                stop()
                return
            }
            let row = page.rowHeight
            let band = row * 1.5
            guard band > 0 else { return }
            let top = point.y
            let bottom = page.bounds.height - point.y
            guard top < band || bottom < band else { return }
            // Measured, so a dropped frame skips ahead instead of slowing.
            let now = link.timestamp
            let dt = CGFloat(min(now - (lastTick ?? now - link.duration), 0.05))
            lastTick = now
            guard dt.isFinite, dt > 0 else { return }
            let gain = { (inside: CGFloat) -> CGFloat in min((band - inside) / band, 3) }
            let speed = 30 * row
            var delta: CGFloat = 0
            if top < band {
                delta -= gain(top) * speed * dt
            }
            if bottom < band {
                delta += gain(bottom) * speed * dt
            }
            if delta != 0 {
                page.crawl(by: delta)
            }
        }
    }

    /// Takes a touch that lands on the scrollbar at touch-down and drives the
    /// thumb with it. Fails at once anywhere else, so nothing waits.
    final class ScrollbarDragRecognizer: UIGestureRecognizer {
        var isOnScrollbar: (CGPoint) -> Bool = { _ in false }
        var begin: (CGPoint) -> Void = { _ in }
        var drag: (CGFloat) -> Void = { _ in }
        private var last: CGPoint?

        override init(target: Any?, action: Selector?) {
            super.init(target: target, action: action)
            delaysTouchesBegan = false
            delaysTouchesEnded = false
        }

        convenience init() {
            self.init(target: nil, action: nil)
        }

        override func touchesBegan(_ touches: Set<UITouch>, with _: UIEvent) {
            guard state == .possible, last == nil, touches.count == 1, let touch = touches.first else {
                state = .failed
                return
            }
            let point = touch.location(in: view)
            guard isOnScrollbar(point) else {
                state = .failed
                return
            }
            last = point
            begin(point)
            state = .began
        }

        override func touchesMoved(_ touches: Set<UITouch>, with _: UIEvent) {
            guard let last, state == .began || state == .changed, let touch = touches.first else { return }
            let point = touch.location(in: view)
            drag(point.y - last.y)
            self.last = point
            state = .changed
        }

        override func touchesEnded(_: Set<UITouch>, with _: UIEvent) {
            state = state == .began || state == .changed ? .ended : .failed
        }

        override func touchesCancelled(_: Set<UITouch>, with _: UIEvent) {
            state = state == .began || state == .changed ? .cancelled : .failed
        }

        override func reset() {
            last = nil
        }
    }

    /// Beneath the text view, taking the touches the text view declines:
    /// those on the editor's touch targets. Popups stay with the editor's own
    /// touch path beneath the page.
    final class TapLayer: UIView {
        weak var page: TextPage?
        let tap = UITapGestureRecognizer()

        init(page: TextPage) {
            self.page = page
            super.init(frame: .zero)
            backgroundColor = .clear
            tap.addTarget(self, action: #selector(handleTap(_:)))
            addGestureRecognizer(tap)
        }

        @available(*, unavailable)
        required init?(coder _: NSCoder) {
            fatalError("init(coder:) has not been implemented")
        }

        override func point(inside point: CGPoint, with event: UIEvent?) -> Bool {
            guard super.point(inside: point, with: event), let page else { return false }
            let kind = page.touchTarget(at: convert(point, to: page))
            return kind != 0 && kind != 3
        }

        @objc private func handleTap(_ tap: UITapGestureRecognizer) {
            guard let page else { return }
            page.tap(at: tap.location(in: page))
        }
    }
#endif
