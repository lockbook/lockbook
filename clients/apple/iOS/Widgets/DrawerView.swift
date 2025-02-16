import SwiftUI

public struct DrawerView<Menu: View, Content: View>: View {

    @State var isOpened: Bool
    @ViewBuilder let menu: Menu
    @ViewBuilder let content: Content

    public var body: some View {
        ZStack(alignment: .leading) {
            NavigationView {
                content
                    .environment(\.isSidebarOpen, isOpened)
                    .toolbar {
                        ToolbarItem(placement: .navigationBarLeading) {
                            Button(action: {
                                isOpened.toggle()
                            }) {
                                Image(systemName: "sidebar.left")
                                    .imageScale(.large)
                            }
                        }
                    }
            }
            
            if isOpened {
                Color
                    .gray
                    .opacity(0.1)
                    .contentShape(Rectangle())
                    .onTapGesture {
                        if isOpened {
                            isOpened.toggle()
                        }
                    }
                
                NavigationView {
                    menu
                }
                    .transition(.move(edge: .leading))
                    .padding(.trailing, 50)
                    .zIndex(1)
                    .environment(\.isSidebarOpen, isOpened)
            }
        }
        .animation(.spring(), value: isOpened)
    }
}

#Preview("Drawer Open") {
    DrawerView(isOpened: true, menu: {
        Color.blue
    }, content: {
        Color.red
    })
}

#Preview("Drawer Closed") {
    DrawerView(isOpened: false, menu: {
        Color.blue
    }, content: {
        Color.red
    })
}

