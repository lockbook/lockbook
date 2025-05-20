import SwiftUI

struct NewDrawerView<Main: View, Side: View>: View {
    @ObservedObject var homeState: HomeState
    
    @ViewBuilder let mainView: Main
    @ViewBuilder let sideView: Side
        
    var dragActivationX: CGFloat = 20
    var velocityActivationX: CGFloat = 300

    @State var offset: CGFloat = 0
    @GestureState var gestureOffset: CGFloat = 0

    func sidebarWidth(width: CGFloat) -> CGFloat {
        return width - 50
    }
    
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
                            
                            UIApplication.shared.sendAction(#selector(UIResponder.resignFirstResponder), to: nil, from: nil, for: nil)
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
                .frame(width:  sidebarWidth(width: geometry.size.width))
                    .animation(.interactiveSpring(
                        response: 0.5,
                        dampingFraction: 0.8,
                        blendDuration: 0),
                               value: gestureOffset
                    )
                    .offset(x: -sidebarWidth(width: geometry.size.width))
                    .offset(x: max(self.offset + self.gestureOffset, 0))
            }
            .onReceive(homeState.$constrainedSidebarState) { newValue in
                withAnimation {
                    if newValue == .openPartial {
                        offset = sidebarWidth(width: geometry.size.width)
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
                                out = min(value.translation.width, sidebarWidth(width: geometry.size.width))
                            }
                        }
                        .onEnded { value in
                            onEnd(value: value, sidebarWidth: sidebarWidth(width: geometry.size.width))
                        }
                )
                .disabled(homeState.constrainedSidebarState == .openPartial)
        }
    }

    func onEnd(value: DragGesture.Value, sidebarWidth: CGFloat){
        let translation = value.translation.width
                
        if (translation > 0 && translation > (sidebarWidth * 0.6)) || value.velocity.width > velocityActivationX {
            offset = sidebarWidth
            homeState.constrainedSidebarState = .openPartial
            UIApplication.shared.sendAction(#selector(UIResponder.resignFirstResponder), to: nil, from: nil, for: nil)
        } else if -translation > (sidebarWidth / 2) {
            offset = 0
            homeState.constrainedSidebarState = .closed
        } else {
            if offset == 0 || homeState.constrainedSidebarState == .closed {
                return
            }
            offset = sidebarWidth
            homeState.constrainedSidebarState = .openPartial
            UIApplication.shared.sendAction(#selector(UIResponder.resignFirstResponder), to: nil, from: nil, for: nil)
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
