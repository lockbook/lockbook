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
                                homeState.constrainedSidebarState = .openPartial
                                UIApplication.shared.sendAction(#selector(UIResponder.resignFirstResponder), to: nil, from: nil, for: nil)
                            }) {
                                Image(systemName: "sidebar.left")
                                    .imageScale(.large)
                            }
                        }
                    }
            }
            
            if homeState.constrainedSidebarState != .closed {
                Color
                    .gray
                    .opacity(0.1)
                    .contentShape(Rectangle())
                    .onTapGesture {
                        if homeState.constrainedSidebarState != .closed {
                            homeState.constrainedSidebarState = .closed
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
        .animation(.spring(duration: 0.2), value: homeState.constrainedSidebarState)
    }
}



/*
 public struct DrawerView<Menu: View, Content: View>: View {

     @ObservedObject var homeState: HomeState
     @ViewBuilder let menu: Menu
     @ViewBuilder let content: Content
     
     @State private var dragOffset: CGFloat = 0
     @State private var lastLocationX: CGFloat? = nil

     public var body: some View {
         ZStack(alignment: .leading) {
             NavigationView {
                 content
                     .toolbar {
                         ToolbarItem(placement: .navigationBarLeading) {
                             Button(action: {
                                 homeState.isConstrainedSidebarOpen.toggle()
                                 print("CLICKING TO TOGGLE: \(homeState.isConstrainedSidebarOpen)")
                                 UIApplication.shared.sendAction(#selector(UIResponder.resignFirstResponder), to: nil, from: nil, for: nil)
                             }) {
                                 Image(systemName: "sidebar.left")
                                     .imageScale(.large)
                             }
                         }
                     }
             }
             
             Color
                 .gray
                 .opacity(homeState.isConstrainedSidebarOpen ? 0.1 : 0)
                 .contentShape(Rectangle())
                 .onTapGesture {
                     if homeState.isConstrainedSidebarOpen {
                         homeState.isConstrainedSidebarOpen.toggle()
                     }
                 }
                 .allowsHitTesting(homeState.isConstrainedSidebarOpen)
             
             let _ = print("checking.. \(homeState.isConstrainedSidebarOpen)")
             
             HStack(spacing: 0) {
                 NavigationView {
                     menu
                 }
                 .transition(.move(edge: .leading))
                 .frame(width: 300)
                 .zIndex(1)
                 
                 Rectangle()
                     .frame(width: 10, height: .infinity)
                     .gesture(
                         DragGesture()
                             .onChanged { newValue in
                                 guard let lastLocationX else {
                                     lastLocationX = newValue.location.x
                                     return
                                 }
                                 
                                 let defaultOffset = homeState.isConstrainedSidebarOpen ? 0.0 : -300.0
                                 
                                 withAnimation {
                                     dragOffset = defaultOffset + (lastLocationX + newValue.location.x)
                                 }
                                 
                                 print("offset changing \(newValue.translation.width)")
                             }
                             .onEnded { _ in
                                 homeState.isConstrainedSidebarOpen = dragOffset > -150
                             }
                     )
             }
             .onChange(of: homeState.isConstrainedSidebarOpen) { newValue in
                 withAnimation(.spring(duration: 0.2)) {
                     dragOffset = newValue ? 0 : -300
                 }
             }
             .offset(x: dragOffset)
         }
     }
 }

 */

#Preview("Drawer") {
    DrawerView(homeState: HomeState(), menu: {
        Color.accentColor
    }, content: {
        Color.red
    })
}
