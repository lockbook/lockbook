#if os(iOS)
    import Bridge
    import UIKit

    /// The page of text: the text input view and the recognizers that
    /// decide the page's touches. The page's own pan and coast-stop see every
    /// touch on the page, so a drag scrolls and a touch during a coast stops
    /// it, beside whatever UIKit's text gestures make of the same touch.
    final class TextPage: UIView {
        unowned let mtkView: iOSMTK
        var wsHandle: UnsafeMutableRawPointer? { mtkView.wsHandle }

        /// A pending selection commits once no finger is on the page.
        let touches = TouchWatch()
        private(set) var text: TextInputView!
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


        init(mtkView: iOSMTK) {
            self.mtkView = mtkView
            super.init(frame: .zero)
            backgroundColor = .clear
            clipsToBounds = true

            text = TextInputView(mtkView: mtkView, page: self)
            text.frame = bounds
            text.autoresizingMask = [.flexibleWidth, .flexibleHeight]
            addSubview(text)

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

            touches.delegate = self
            addGestureRecognizer(touches)
        }

        @available(*, unavailable)
        required init?(coder _: NSCoder) {
            fatalError("init(coder:) has not been implemented")
        }

        /// The page itself takes no touches: they belong to the text view or
        /// whatever lies beneath (popups, the toolbar band).
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
        }

        func documentReplaced() {
            text.documentReplaced()
        }

        // MARK: - Page scroll

        /// UIKit's handle drag and loupe drag, seen once they share a touch
        /// with the page's recognizers. A target only observes.
        fileprivate func observeTextDrag(_ recognizer: UIGestureRecognizer) {
            guard recognizer.isRangeAdjustment || recognizer.isCaretDrag || recognizer.isSelectionDrag,
                  !observedDrags.contains(where: { $0 === recognizer })
            else { return }
            observedDrags.append(recognizer)
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
                return otherGestureRecognizer === scrollPan || otherGestureRecognizer.isRangeAdjustment
                    || otherGestureRecognizer.isSelectionDrag
            }
            return false
        }
    }

#endif
