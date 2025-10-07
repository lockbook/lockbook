import SwiftUI
import SwiftWorkspace

struct NewDrawerView<Main: View, Side: View>: View {
    @ObservedObject var homeState: HomeState
    
    @ViewBuilder let mainView: Main
    @ViewBuilder let sideView: Side
        
    let dragActivationClosedX: CGFloat = 20
    let dragActivationOpenX: CGFloat = 50
    
    let velocityActivationX: CGFloat = 300
    let successThreshold: CGFloat = 0.6
    
    let sidebarTrailingPadding: CGFloat = 50

    @State var offset: CGFloat = 0
    @GestureState var gestureOffset: CGFloat = 0

    func sidebarWidth(width: CGFloat) -> CGFloat {
        return width - sidebarTrailingPadding
    }
    
    var body: some View {
        GeometryReader { geometry in
            ZStack(alignment: .leading) {
                NavigationView {
                    mainView
                        .toolbar {
                            ToolbarItem(placement: .navigationBarLeading) {
                                Button(action: {
                                    homeState.sidebarState = .open
                                    
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
                                   value: homeState.sidebarState == .open)
                        .onTapGesture {
                            withAnimation {
                                if homeState.sidebarState == .open {
                                    homeState.sidebarState = .closed
                                } else {
                                    homeState.sidebarState = .open
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
                                    homeState.sidebarState = .closed
                                }) {
                                    Image(systemName: "sidebar.left")
                                        .imageScale(.large)
                                }
                            }
                        }
                }
                .frame(width: sidebarWidth(width: geometry.size.width))
                    .animation(.interactiveSpring(
                        response: 0.5,
                        dampingFraction: 0.8,
                        blendDuration: 0),
                               value: gestureOffset
                    )
                    .offset(x: min(-sidebarWidth(width: geometry.size.width) + max(self.offset + self.gestureOffset, 0), 0))
            }
            .onReceive(homeState.$sidebarState) { newValue in
                withAnimation {
                    if newValue == .open {
                        offset = sidebarWidth(width: geometry.size.width)
                    } else {
                        offset = 0
                    }
                }
            }
            
            if homeState.sidebarState == .closed {
                Rectangle()
                    .fill(Color.clear)
                    .frame(width: dragActivationClosedX)
                    .contentShape(Rectangle())
                    .gesture(
                        DragGesture()
                            .updating($gestureOffset) { value, out, _ in
                                out = min(value.translation.width, sidebarWidth(width: geometry.size.width))
                            }
                            .onEnded { value in
                                onOpenEnd(translation: value.translation.width, velocity: value.velocity.width, sidebarWidth: sidebarWidth(width: geometry.size.width))
                            }
                    )
            }
            
            if homeState.sidebarState == .open {
                Rectangle()
                    .fill(Color.clear)
                    .frame(width: dragActivationClosedX)
                    .contentShape(Rectangle())
                    .gesture(
                        DragGesture()
                            .updating($gestureOffset) { value, out, _ in
                                out = max(value.translation.width, -sidebarWidth(width: geometry.size.width))
                            }
                            .onEnded { value in
                                onCloseEnd(translation: value.translation.width, velocity: value.velocity.width, sidebarWidth: sidebarWidth(width: geometry.size.width))
                            }
                    )
                    .frame(maxWidth: .infinity, alignment: .trailing)
                    .padding(.trailing, sidebarTrailingPadding - 20)
            }
        }
    }
    
    func onOpenEnd(translation: CGFloat, velocity: CGFloat, sidebarWidth: CGFloat){
        let isOpenEnough = translation > 0 && translation > (sidebarWidth * successThreshold)
        let isFastEnough = velocity > velocityActivationX
                
        if isOpenEnough || isFastEnough {
            offset = sidebarWidth
            homeState.sidebarState = .open
            UIApplication.shared.sendAction(#selector(UIResponder.resignFirstResponder), to: nil, from: nil, for: nil)
        } else {
            offset = 0
            homeState.sidebarState = .closed
        }
    }
    
    func onCloseEnd(translation: CGFloat, velocity: CGFloat, sidebarWidth: CGFloat) {
        let translation = abs(translation)
        let velocity = abs(velocity)
        
        let isOpenEnough = translation > 0 && translation > (sidebarWidth * successThreshold)
        let isFastEnough = velocity > velocityActivationX
                
        if isOpenEnough || isFastEnough {
            offset = 0
            homeState.sidebarState = .closed
            UIApplication.shared.sendAction(#selector(UIResponder.resignFirstResponder), to: nil, from: nil, for: nil)
        } else {
            offset = sidebarWidth
            homeState.sidebarState = .open
        }
    }

    func getBlurRadius() -> CGFloat {
        return (offset + gestureOffset) / (UIScreen.main.bounds.height * 0.50)
    }
}

#Preview {
    NewDrawerView(homeState: HomeState(workspaceOutput: .preview, filesModel: .preview), mainView: {
        Color.blue
    }, sideView: {
        Color.red
    })
}
