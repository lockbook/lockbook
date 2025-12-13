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
            tabContent(selectedTab)
        }
        .toolbar {
            ToolbarItemGroup(
                placement: toolbarItemPlacement,
                content: {
                    TabPicker(
                        selectedTab: $selectedTab
                    )
                }
            )
        }
    }
}

