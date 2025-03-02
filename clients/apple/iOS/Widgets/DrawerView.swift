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
                                homeState.isConstrainedSidebarOpen.toggle()
                            }) {
                                Image(systemName: "sidebar.left")
                                    .imageScale(.large)
                            }
                        }
                    }
            }
            
            if homeState.isConstrainedSidebarOpen {
                Color
                    .gray
                    .opacity(0.1)
                    .contentShape(Rectangle())
                    .onTapGesture {
                        if homeState.isConstrainedSidebarOpen {
                            homeState.isConstrainedSidebarOpen.toggle()
                        }
                    }
                
                NavigationView {
                    menu
                }
                    .transition(.move(edge: .leading))
                    .padding(.trailing, 50)
                    .zIndex(1)
            }
        }
        .animation(.spring(), value: homeState.isConstrainedSidebarOpen)
    }
}

#Preview("Drawer") {
    DrawerView(homeState: HomeState(workspaceState: WorkspaceState()), menu: {
        Color.blue
    }, content: {
        Color.red
    })
}
