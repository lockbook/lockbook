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

            if homeState.sidebarState == .closed {
                Rectangle()
                    .fill(Color.clear)
                    .frame(width: Constants.dragActivationClosedX)
                    .contentShape(Rectangle())
                    .gesture(
                        DragGesture()
                            .onChanged { value in
                                setSidebarOffset(
                                    newOffset: min(
                                        value.translation.width,
                                        calculatedSidebarWidth
                                    )
                                )
                            }
                            .onEnded { value in
                                onOpenEnd(
                                    velocity: value.velocity.width,
                                    sidebarWidth: calculatedSidebarWidth
                                )
                            }
                    )
            }

            if homeState.sidebarState == .open {
                Rectangle()
                    .fill(Color.clear)
                    .frame(width: Constants.dragActivationClosedX)
                    .contentShape(Rectangle())
                    .gesture(
                        DragGesture()
                            .onChanged { value in
                                setSidebarOffset(
                                    newOffset: calculatedSidebarWidth
                                        + value.translation.width
                                )
                            }
                            .onEnded { value in
                                onCloseEnd(
                                    velocity: value.velocity.width,
                                    sidebarWidth: calculatedSidebarWidth
                                )
                            }
                    )
                    .padding(
                        .leading,
                        calculatedSidebarWidth
                            - (Constants.dragActivationClosedX / 2)
                    )
            }
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

private struct Constants {
    static let dragActivationClosedX: CGFloat = 20
    static let dragActivationOpenX: CGFloat = 50
    static let velocityActivationX: CGFloat = 300
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

struct CloseSidebarToolbarModifier: ViewModifier {
    @EnvironmentObject var homeState: HomeState
    
    func body(content: Content) -> some View {
        content
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
}

extension View {
    func closeSidebarToolbar() -> some View {
        self.modifier(CloseSidebarToolbarModifier())
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
