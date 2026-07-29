import SwiftUI

extension View {
    func onMiddleClick(perform action: @escaping () -> Void) -> some View {
        #if os(macOS)
            overlay {
                MiddleClickCatcher(action: action)
            }
        #else
            self
        #endif
    }
}

#if os(macOS)
    private struct MiddleClickCatcher: NSViewRepresentable {
        let action: () -> Void

        func makeNSView(context: Context) -> MiddleClickView {
            let view = MiddleClickView()
            view.action = action
            return view
        }

        func updateNSView(_ nsView: MiddleClickView, context: Context) {
            nsView.action = action
        }

        final class MiddleClickView: NSView {
            var action: (() -> Void)?

            override func hitTest(_ point: NSPoint) -> NSView? {
                switch NSApp.currentEvent?.type {
                case .otherMouseDown, .otherMouseUp, .otherMouseDragged:
                    super.hitTest(point)
                default:
                    nil
                }
            }

            override func otherMouseDown(with event: NSEvent) {}

            override func otherMouseUp(with event: NSEvent) {
                guard event.buttonNumber == 2,
                      bounds.contains(convert(event.locationInWindow, from: nil))
                else {
                    return
                }

                action?()
            }
        }
    }
#endif
