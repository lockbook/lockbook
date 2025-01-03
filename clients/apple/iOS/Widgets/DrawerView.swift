import SwiftUI

public struct DrawerView<Content: View>: View {

    @State var isOpened: Bool
    @ViewBuilder let menu: Content
    @ViewBuilder let content: Content

    public var body: some View {
        ZStack(alignment: .leading) {
            content
                .environment(\.isSidebarOpen, isOpened)

            if isOpened {
                Color.clear
                    .contentShape(Rectangle())
                    .onTapGesture {
                        if isOpened {
                            isOpened.toggle()
                        }
                    }
                
                menu
                    .transition(.move(edge: .leading))
                    .padding(.trailing, 100)
                    .zIndex(1)
                    .environment(\.isSidebarOpen, isOpened)
            }
        }
        .animation(.spring(), value: isOpened)
        .navigationBarItems(leading: Button(action: {
            isOpened.toggle()
        }) {
            Image(systemName: "sidebar.left")
                .imageScale(.large)
        })
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

