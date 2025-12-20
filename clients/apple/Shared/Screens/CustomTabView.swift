import SwiftUI

struct CustomTabView<TabContent: View>: View {
    @Binding var selectedTab: TabType
    @ViewBuilder var tabContent: (TabType) -> TabContent
    
    var toolbarItemPlacement: ToolbarItemPlacement {
        #if os(macOS)
        .principal
        #else
        .topBarLeading
        #endif
    }
    
    var body: some View {
        Group {
            tabItem
        }
        .toolbar {
            ToolbarItem(
                placement: toolbarItemPlacement,
                content: {
                    TabPicker(
                        selectedTab: $selectedTab
                    )
                }
            )
        }
    }
    
    // To persist state on macOS, I ensure every view is rendered, but unselected
    // views are hidden. This doesn't work well on iOS since navigation titles don't
    // handle this behavior well
    var tabItem: some View {
        Group {
            #if os(iOS)
            tabContent(selectedTab)
            #else
            ZStack {
                ForEach(TabType.allCases) { tabVariant in
                    tabContent(tabVariant)
                        .opacity(selectedTab == tabVariant ? 1 : 0)
                        .allowsHitTesting(selectedTab == tabVariant)
                        .accessibilityHidden(selectedTab != tabVariant)
                }
            }
            #endif
        }
    }
}

