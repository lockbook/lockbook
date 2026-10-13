#if os(iOS)
    import UIKit

    /// Whether any finger is on the page. It never recognizes and never
    /// interferes, so it sees every touch beside every other recognizer.
    final class TouchWatch: UIGestureRecognizer {
        private var down = 0
        var touching: Bool { down > 0 }
        /// The first finger of a touch sequence came down.
        var onFirstTouch: ((CGPoint) -> Void)?

        override init(target: Any?, action: Selector?) {
            super.init(target: target, action: action)
            cancelsTouchesInView = false
            delaysTouchesBegan = false
            delaysTouchesEnded = false
        }

        convenience init() {
            self.init(target: nil, action: nil)
        }

        override func touchesBegan(_ touches: Set<UITouch>, with _: UIEvent) {
            let wasTouching = touching
            down += touches.count
            if !wasTouching, let touch = touches.first {
                onFirstTouch?(touch.location(in: view))
            }
        }

        override func touchesEnded(_ touches: Set<UITouch>, with _: UIEvent) {
            lift(touches)
        }

        override func touchesCancelled(_ touches: Set<UITouch>, with _: UIEvent) {
            lift(touches)
        }

        private func lift(_ touches: Set<UITouch>) {
            down = max(0, down - touches.count)
            if down == 0 {
                state = .failed
            }
        }

        override func reset() {
            down = 0
        }
    }
#endif
