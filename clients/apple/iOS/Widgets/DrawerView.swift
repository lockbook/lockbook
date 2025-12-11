import SwiftUI
import SwiftWorkspace

struct DrawerView<Main: View, Side: View>: View {

    @ObservedObject var homeState: HomeState

    @ViewBuilder let mainView: Main
    @ViewBuilder let sideView: Side

    @State private var sidebarOffset: CGFloat = Constants.sidebarOffsetClosed

    var body: some View {
        GeometryReader { geometry in
            let calculatedSidebarWidth = sidebarWidth(
                width: geometry.size.width
            )

            ZStack(alignment: .leading) {
                NavigationStack {
                    mainView
                        .toolbar {
                            ToolbarItem(placement: .navigationBarLeading) {
                                Button {
                                    homeState.sidebarState = .open
                                    resignFirstResponder()
                                } label: {
                                    Label(
                                        "Sidebar",
                                        systemImage: "sidebar.left"
                                    )
                                    .imageScale(.large)
                                    .labelStyle(.iconOnly)
                                }
                            }
                        }
                }
                .overlay(mainOverlayTapGesture)

            sideView
                .frame(width: calculatedSidebarWidth)
                .offset(
                    x: min(
                        -calculatedSidebarWidth
                            + max(
                                sidebarOffset,
                                Constants.sidebarOffsetClosed
                            ),
                        Constants.sidebarOffsetClosed
                    )
                )
            }
            .onReceive(homeState.$sidebarState) { newValue in
                let newOffset =
                    newValue == .open
                    ? calculatedSidebarWidth
                    : Constants.sidebarOffsetClosed
                setSidebarOffset(newOffset: newOffset)
            }
            .onChange(of: geometry.size) { newSize in
                if homeState.sidebarState == .open {
                    let newSidebarWidth = sidebarWidth(width: newSize.width)
                    setSidebarOffset(
                        newOffset: newSidebarWidth
                    )
                } else {
                    setSidebarOffset(newOffset: Constants.sidebarOffsetClosed)
                }
            }
            .gesture(
                DrawerGesture(
                    geometry: geometry,
                    sidebarWidth: sidebarWidth(width: geometry.size.width),
                    sidebarOffset: $sidebarOffset,
                    onChanged: { value in
                        if homeState.sidebarState == .closed {
                            setSidebarOffset(
                                newOffset: min(
                                    value.translation.x,
                                    calculatedSidebarWidth
                                )
                            )
                        }
                        if homeState.sidebarState == .open {
                            setSidebarOffset(
                                newOffset: calculatedSidebarWidth
                                    + value.translation.x
                            )
                        }
                    },
                    onEnded: { value in
                        if homeState.sidebarState == .closed {
                            onOpenEnd(
                                velocity: value.velocity.x,
                                sidebarWidth: calculatedSidebarWidth
                            )
                        } else if homeState.sidebarState == .open {
                            onCloseEnd(
                                velocity: value.velocity.x,
                                sidebarWidth: calculatedSidebarWidth
                            )
                        }
                    }
                )
            )
        }
        .ignoresSafeArea(.container, edges: .all)
    }

    private func sidebarWidth(width: CGFloat) -> CGFloat {
        let orientationWidth =
            UIDevice.current.orientation.isPortrait
            ? Constants.defaultSidebarWidthPortrait
            : Constants.defaultSidebarWidthLandscape

        let maxAllowedWidth = width - Constants.sidebarTrailingPadding

        return min(orientationWidth, maxAllowedWidth)
    }

    private func setSidebarOffset(newOffset: CGFloat) {
        withAnimation(Constants.sidebarAnimation) {
            self.sidebarOffset = newOffset
        }
    }

    private func resignFirstResponder() {
        UIApplication.shared.sendAction(
            #selector(UIResponder.resignFirstResponder),
            to: nil,
            from: nil,
            for: nil
        )
    }

    private func onOpenEnd(
        velocity: CGFloat,
        sidebarWidth: CGFloat
    ) {
        let isOpenEnough =
            sidebarOffset > (sidebarWidth * Constants.successThreshold)
        let isFastEnough = velocity > Constants.velocityActivationX

        if isOpenEnough || isFastEnough {
            homeState.sidebarState = .open
            resignFirstResponder()
        } else {
            homeState.sidebarState = .closed
        }
    }

    private func onCloseEnd(
        velocity: CGFloat,
        sidebarWidth: CGFloat
    ) {
        let isClosedEnough =
            sidebarOffset < (sidebarWidth * Constants.successThreshold)
        let isFastEnough = -velocity > Constants.velocityActivationX

        if isClosedEnough || isFastEnough {
            homeState.sidebarState = .closed
            resignFirstResponder()
        } else {
            homeState.sidebarState = .open
        }
    }

    private func getBlurRadius() -> CGFloat {
        return sidebarOffset
            / (UIScreen.main.bounds.height * Constants.blurDenominatorFactor)
    }

    private var mainOverlayTapGesture: some View {
        GeometryReader { _ in
            EmptyView()
        }
        .background(.black.opacity(0.6))
        .opacity(getBlurRadius())
        .onTapGesture {
            if homeState.sidebarState == .open {
                homeState.sidebarState = .closed
            } else {
                homeState.sidebarState = .open
            }
            resignFirstResponder()
        }
    }
}

private struct Constants {
    static let velocityActivationX: CGFloat = 300 // fling this fast to open the drawer
    static let successThreshold: CGFloat = 0.6 // drag this far of the way out to open the drawer
    static let activationDistance: CGFloat = 10.0 // must drag at least this far or your drag is actually a tap
    static let activationRatio: CGFloat = 2.0 // must drag at least this horizontally in terms of abs(x) / abs(y)
    static let dragHandleWidth: CGFloat = 100 // must drag starting from within this distance of whichever edge

    static let sidebarTrailingPadding: CGFloat = 50
    static let defaultSidebarWidthPortrait: CGFloat = 350
    static let defaultSidebarWidthLandscape: CGFloat = 500
    static let animationResponse: Double = 0.3
    static let animationDampingFraction: Double = 0.8
    static let animationBlendDuration: Double = 0
    static let blurDenominatorFactor: CGFloat = 0.50
    static let sidebarOffsetClosed: CGFloat = 0

    static var sidebarAnimation: Animation {
        .interactiveSpring(
            response: Constants.animationResponse,
            dampingFraction: Constants.animationDampingFraction,
            blendDuration: Constants.animationBlendDuration
        )
    }
}

// MARK: - DragValue
struct DragValue {
    let location: CGPoint
    let translation: CGPoint
    let velocity: CGPoint
}

// MARK: - DrawerGesture
/// Like a DragGesture, but:
/// * takes priority over gestures in subviews
/// * must be a horizontal drag to activate
struct DrawerGesture: UIGestureRecognizerRepresentable {
    typealias UIGestureRecognizerType = Recognizer

    var geometry: GeometryProxy
    var sidebarWidth: CGFloat
    @Binding var sidebarOffset: CGFloat

    let onChanged: (DragValue) -> Void
    let onEnded: (DragValue) -> Void

    func makeCoordinator(converter: CoordinateSpaceConverter) -> Coordinator {
        Coordinator(
            onChanged: onChanged,
            onEnded: onEnded
        )
    }

    @MainActor func makeUIGestureRecognizer(context: Context) -> UIGestureRecognizerType {
        let recognizer = Recognizer(geometry: geometry, sidebarWidth: sidebarWidth, sidebarOffset: $sidebarOffset)
        recognizer.delegate = context.coordinator
        return recognizer
    }

    @MainActor func updateUIGestureRecognizer(
        _ uiGestureRecognizer: UIGestureRecognizerType,
        context: Context
    ) {}

    @MainActor func handleUIGestureRecognizerAction(
        _ recognizer: UIGestureRecognizerType,
        context: Context
    ) {
        context.coordinator.handle(recognizer)
    }

    // MARK: - DrawerGesture.Coordinator
    class Coordinator: NSObject, UIGestureRecognizerDelegate {
        let onChanged: (DragValue) -> Void
        let onEnded: (DragValue) -> Void

        init(
            onChanged: @escaping (DragValue) -> Void,
            onEnded: @escaping (DragValue) -> Void
        ) {
            self.onChanged = onChanged
            self.onEnded = onEnded
        }

        @objc func handle(_ gesture: UIGestureRecognizerType) {
            guard let view = gesture.view else { return }

            let location = gesture.location(in: view)
            let translation = gesture.translation(in: view)
            let velocity = gesture.velocity(in: view)

            let value = DragValue(
                location: location,
                translation: translation,
                velocity: velocity
            )

            switch gesture.state {
            case .began:
                break
            case .changed:
                onChanged(value)
            case .ended, .cancelled:
                onEnded(value)
            default:
                break
            }
        }

        // fills the "takes priority over gestures in subviews" requirement
        func gestureRecognizer(
            _ gestureRecognizer: UIGestureRecognizer,
            shouldBeRequiredToFailBy otherGestureRecognizer: UIGestureRecognizer
        ) -> Bool {
            guard let view = gestureRecognizer.view,
                let otherView = otherGestureRecognizer.view
            else { return false }

            return otherView.isDescendant(of: view)
        }
    }

    // MARK: - DrawerGesture.Recognizer
    final class Recognizer: UIGestureRecognizer {
        var geometry: GeometryProxy
        var sidebarWidth: CGFloat
        @Binding var sidebarOffset: CGFloat

        // continuously tracked state (reset upon completion)
        private var startLocation: CGPoint = .zero
        private var lastLocation: CGPoint = .zero
        private var lastTimestamp: TimeInterval = 0

        // outputs (available via accessors, including after completion)
        private var translation: CGPoint = .zero
        private var velocity: CGPoint = .zero

        init(geometry: GeometryProxy, sidebarWidth: CGFloat, sidebarOffset: Binding<CGFloat>) {
            self.geometry = geometry
            self.sidebarWidth = sidebarWidth
            _sidebarOffset = sidebarOffset
            super.init(target: nil, action: nil)
            self.name = "DrawerGesture.Recognizer"
        }

        override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent) {
            guard state == .possible else { return }
            guard let touch = touches.first else { return }

            let location = touch.location(in: self.view)

            startLocation = location
            lastLocation = location
            lastTimestamp = touch.timestamp

            translation = .zero
            velocity = .zero

            // open: drag from right side; closed: drag from left side
            let isOpen = sidebarOffset > sidebarWidth * Constants.successThreshold
            if isOpen {
                if location.x < geometry.size.width - Constants.dragHandleWidth {
                    state = .failed
                }
            } else {
                if location.x > Constants.dragHandleWidth {
                    state = .failed
                }
            }
        }

        override func touchesMoved(_ touches: Set<UITouch>, with event: UIEvent) {
            guard let touch = touches.first else { return }
            guard let view = self.view else { return }

            let currentLocation = touch.location(in: view)
            let currentTimestamp = touch.timestamp

            let deltaSinceFirst = CGPoint(
                x: currentLocation.x - startLocation.x,
                y: currentLocation.y - startLocation.y)
            let deltaSinceLast = CGPoint(
                x: currentLocation.x - lastLocation.x,
                y: currentLocation.y - lastLocation.y)
            let timeDelta = currentTimestamp - lastTimestamp

            lastLocation = currentLocation
            lastTimestamp = currentTimestamp

            translation = deltaSinceFirst
            if timeDelta > 0 {
                velocity = CGPoint(
                    x: deltaSinceLast.x / CGFloat(timeDelta),
                    y: deltaSinceLast.y / CGFloat(timeDelta))
            }

            // begin after min distance; fail instantly if too vertical
            // fills the "must be a horizontal drag to activate" requirement
            if state == .possible {
                if abs(deltaSinceLast.y) > abs(deltaSinceLast.x) * Constants.activationRatio {
                    state = .failed
                } else if hypot(deltaSinceLast.x, deltaSinceLast.y) > Constants.activationDistance {
                    state = .began
                }
            } else if state == .began || state == .changed {
                state = .changed
            } else {
                state = .failed
            }
        }

        override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent) {
            if state == .began || state == .changed {
                state = .ended
            } else {
                state = .failed
            }
            reset()
        }

        override func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent) {
            if state == .began || state == .changed {
                state = .cancelled
            } else {
                state = .failed
            }
            reset()
        }

        override func reset() {
            super.reset()
            startLocation = .zero
            lastLocation = .zero
            lastTimestamp = 0
        }

        func translation(in _: UIView?) -> CGPoint {
            translation
        }

        func velocity(in _: UIView?) -> CGPoint {
            velocity
        }

        override func canBePrevented(by preventingGestureRecognizer: UIGestureRecognizer) -> Bool {
            false
        }

        override func canPrevent(_ preventedGestureRecognizer: UIGestureRecognizer) -> Bool {
            false
        }
    }
}

#Preview {
    DrawerView(
        homeState: HomeState(workspaceOutput: .preview, filesModel: .preview),
        mainView: {
            Color.blue
        },
        sideView: {
            Color.red
        }
    )
}
