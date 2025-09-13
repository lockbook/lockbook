import SwiftUI
import SwiftWorkspace

public struct DrawerView<Menu: View, Content: View>: View {

    @ObservedObject var homeState: HomeState
    @ViewBuilder let menu: Menu
    @ViewBuilder let content: Content

    public var body: some View {
        ZStack(alignment: .leading) {
            NavigationView {
                content
                    .toolbar {
                        ToolbarItem(placement: .navigationBarLeading) {
                            Button(action: {
                                homeState.compactSidebarState = .openPartial
                                UIApplication.shared.sendAction(#selector(UIResponder.resignFirstResponder), to: nil, from: nil, for: nil)
                            }) {
                                Image(systemName: "sidebar.left")
                                    .imageScale(.large)
                            }
                        }
                    }
            }
            
            if homeState.compactSidebarState != .closed {
                Color
                    .gray
                    .opacity(0.1)
                    .contentShape(Rectangle())
                    .onTapGesture {
                        if homeState.compactSidebarState != .closed {
                            homeState.compactSidebarState = .closed
                        }
                    }
                
                NavigationView {
                    menu
                }
                    .transition(.move(edge: .leading))
//                    .frame(width: homeState.constrainedSidebarState == .openExpanded ? .infinity : 300)
                    .zIndex(1)
            }
        }
        .animation(.spring(duration: 0.2), value: homeState.compactSidebarState)
    }
}

#Preview("Drawer") {
    DrawerView(homeState: HomeState(), menu: {
        Color.accentColor
    }, content: {
        Color.red
    })
}
