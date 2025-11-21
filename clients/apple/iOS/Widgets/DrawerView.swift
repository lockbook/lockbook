import SwiftUI
import SwiftWorkspace

// MARK: - DrawerView
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

            let dragClosedMinX = calculatedSidebarWidth - Constants.dragActivationX / 2
            let dragClosedMaxX = calculatedSidebarWidth + Constants.dragActivationX / 2
            let dragOpenMinX: CGFloat = 0
            let dragOpenMaxX = Constants.dragActivationX

            ZStack(alignment: .leading) {
                NavigationView {
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

                NavigationView {
                    sideView
                        .toolbar {
                            ToolbarItem(placement: .navigationBarLeading) {
                                Button {
                                    homeState.sidebarState = .closed
                                } label: {
                                    Image(systemName: "sidebar.left")
                                        .imageScale(.large)
                                }
                            }
                        }
                }
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
                NonCancellingDragGesture(minimumDistance: 0) { value in
                    if homeState.sidebarState == .closed {
                        if dragOpenMinX < value.startLocation.x && value.startLocation.x < dragOpenMaxX {
                            setSidebarOffset(
                                newOffset: min(
                                    value.translation.width,
                                    calculatedSidebarWidth
                                )
                            )
                        }
                    } else {
                        if dragClosedMinX < value.startLocation.x && value.startLocation.x < dragClosedMaxX {
                            setSidebarOffset(
                                newOffset: calculatedSidebarWidth
                                + value.translation.width
                            )
                        }
                    }
                } onEnded: { value in
                    if homeState.sidebarState == .closed {
                        if dragOpenMinX < value.startLocation.x && value.startLocation.x < dragOpenMaxX {
                            onOpenEnd(
                                velocity: value.velocity.dx,
                                sidebarWidth: calculatedSidebarWidth
                            )
                        }
                    } else {
                        if dragClosedMinX < value.startLocation.x && value.startLocation.x < dragClosedMaxX {
                            onCloseEnd(
                                velocity: value.velocity.dx,
                                sidebarWidth: calculatedSidebarWidth
                            )
                        }
                    }
                }
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
        let absoluteVelocity = abs(velocity)

        let isClosedEnough =
            sidebarOffset < (sidebarWidth * Constants.successThreshold)
        let isFastEnough = absoluteVelocity > Constants.velocityActivationX

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

// MARK: - DragValue
struct DragValue {
    let location: CGPoint
    let startLocation: CGPoint
    let translation: CGSize
    let velocity: CGVector
}

// MARK: - NonCancellingDragGesture
/// Like a DragGesture, but with `cancelsTouchesInView = false`
struct NonCancellingDragGesture: UIGestureRecognizerRepresentable {
    typealias UIGestureRecognizerType = UIPanGestureRecognizer

    let minimumDistance: CGFloat
    let onChanged: (DragValue) -> Void
    let onEnded: (DragValue) -> Void

    func makeCoordinator(converter: CoordinateSpaceConverter) -> Coordinator {
        Coordinator(
            minimumDistance: minimumDistance,
            onChanged: onChanged,
            onEnded: onEnded
        )
    }

    @MainActor func makeUIGestureRecognizer(context: Context) -> UIGestureRecognizerType {
        let recognizer = UIPanGestureRecognizer()
        recognizer.cancelsTouchesInView = false // <- the only meaningful line in this struct
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

    class Coordinator: NSObject, UIGestureRecognizerDelegate {
        let minimumDistance: CGFloat
        let onChanged: (DragValue) -> Void
        let onEnded: (DragValue) -> Void
        var startLocation: CGPoint?

        init(minimumDistance: CGFloat, onChanged: @escaping (DragValue) -> Void, onEnded: @escaping (DragValue) -> Void) {
            self.minimumDistance = minimumDistance
            self.onChanged = onChanged
            self.onEnded = onEnded
        }

        @objc func handle(_ gesture: UIPanGestureRecognizer) {
            guard let view = gesture.view else { return }

            let location = gesture.location(in: view)
            let translation = gesture.translation(in: view)

            switch gesture.state {
            case .began:
                startLocation = location
            case .changed:
                guard let start = startLocation else { return }
                let distance = hypot(location.x - start.x, location.y - start.y)
                guard distance >= minimumDistance else { return }

                let value = DragValue(
                    location: location,
                    startLocation: start,
                    translation: CGSize(width: translation.x, height: translation.y),
                    velocity: CGVector()
                )
                onChanged(value)
            case .ended, .cancelled:
                guard let start = startLocation else { return }
                let velocityValue = gesture.velocity(in: view)

                let value = DragValue(
                    location: location,
                    startLocation: start,
                    translation: CGSize(width: translation.x, height: translation.y),
                    velocity: CGVector(dx: velocityValue.x, dy: velocityValue.y)
                )
                onEnded(value)
                startLocation = nil
            default:
                break
            }
        }
    }
}

private struct Constants {
    static let dragActivationX: CGFloat = 100
    static let velocityActivationX: CGFloat = 500
    static let successThreshold: CGFloat = 0.6
    static let sidebarTrailingPadding: CGFloat = 50
    static let defaultSidebarWidthPortrait: CGFloat = 350
    static let defaultSidebarWidthLandscape: CGFloat = 500
    static let animationResponse: Double = 0.5
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
