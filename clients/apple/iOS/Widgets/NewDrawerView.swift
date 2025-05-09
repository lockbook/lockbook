import SwiftUI

struct NewDrawerView<Main: View, Side: View>: View {
    @ObservedObject var homeState: HomeState
    
    @ViewBuilder let mainView: Main
    @ViewBuilder let sideView: Side
    
    var sideBarWidth = UIScreen.main.bounds.size.width
    
    var dragActivationX: CGFloat = 20
    var velocityActivationX: CGFloat = 300

    @State var offset: CGFloat = 0
    @GestureState var gestureOffset: CGFloat = 0

    var body: some View {
        GeometryReader { geometry in
            ZStack(alignment: .leading) {
                NavigationView {
                    mainView
                        .toolbar {
                            ToolbarItem(placement: .navigationBarLeading) {
                                Button(action: {
                                    homeState.constrainedSidebarState = .openPartial
                                    UIApplication.shared.sendAction(#selector(UIResponder.resignFirstResponder), to: nil, from: nil, for: nil)
                                }) {
                                    Image(systemName: "sidebar.left")
                                        .imageScale(.large)
                                }
                            }
                        }
                }
                    .animation(.interactiveSpring(
                        response: 0.5,
                        dampingFraction: 0.8,
                        blendDuration: 0),
                               value: gestureOffset
                    )
                    .overlay(
                        GeometryReader { _ in
                            EmptyView()
                        }
                        .background(.black.opacity(0.6))
                        .opacity(getBlurRadius())
                        .animation(.interactiveSpring(
                            response: 0.5,
                            dampingFraction: 0.8,
                            blendDuration: 0),
                                   value: homeState.constrainedSidebarState == .openPartial)
                        .onTapGesture {
                            withAnimation {
                                if homeState.constrainedSidebarState == .openPartial {
                                    homeState.constrainedSidebarState = .closed
                                } else {
                                    homeState.constrainedSidebarState = .openPartial
                                }
                            }
                        }
                    )
                
                NavigationView {
                    sideView
                        .toolbar {
                            ToolbarItem(placement: .navigationBarLeading) {
                                Button(action: {
                                    homeState.constrainedSidebarState = .closed
                                }) {
                                    Image(systemName: "sidebar.left")
                                        .imageScale(.large)
                                }
                            }
                        }
                }
                    .frame(width:  sideBarWidth)
                    .animation(.interactiveSpring(
                        response: 0.5,
                        dampingFraction: 0.8,
                        blendDuration: 0),
                               value: gestureOffset
                    )
                    .offset(x: -sideBarWidth)
                    .offset(x: max(self.offset + self.gestureOffset, 0))
            }
            .onReceive(homeState.$constrainedSidebarState) { newValue in
                print("setting offset for \(newValue)")
                withAnimation {
                    if newValue == .openPartial {
                        offset = sideBarWidth
                    } else {
                        offset = 0
                    }
                }
            }
            
            Rectangle()
                .fill(Color.clear)
                .frame(width: dragActivationX)
                .contentShape(Rectangle())
                .gesture(
                    DragGesture()
                        .updating($gestureOffset) { value, out, _ in
                            if value.translation.width > 0 && homeState.constrainedSidebarState == .openPartial {
                                out = value.translation.width * 3
                            } else {
                                out = min(value.translation.width, sideBarWidth)
                            }
                        }
                        .onEnded(onEnd)
                )
                .disabled(homeState.constrainedSidebarState == .openPartial)
        }
    }

    func onEnd(value: DragGesture.Value){
        let translation = value.translation.width
                
        if (translation > 0 && translation > (sideBarWidth * 0.6)) || value.velocity.width > velocityActivationX {
            offset = sideBarWidth
            homeState.constrainedSidebarState = .openPartial
        } else if -translation > (sideBarWidth / 2) {
            offset = 0
            homeState.constrainedSidebarState = .closed
        } else {
            if offset == 0 || homeState.constrainedSidebarState == .closed {
                return
            }
            offset = sideBarWidth
            homeState.constrainedSidebarState = .openPartial
        }
    }

    func getBlurRadius() -> CGFloat {
        let progress =  (offset + gestureOffset) / (UIScreen.main.bounds.height * 0.50)
        return progress
    }
}

#Preview {
    NewDrawerView(homeState: HomeState(), mainView: {
        Color.blue
    }, sideView: {
        Color.red
    })
}
