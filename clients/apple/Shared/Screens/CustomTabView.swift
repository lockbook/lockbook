import SwiftUI

struct CustomTabView<TabContent: View>: View {
    @Binding var selectedTab: TabType
    @ViewBuilder var tabContent: (TabType) -> TabContent
    
    var body: some View {
        ZStack {
            ForEach(TabType.allCases) { mode in
                tabContent(mode)
                    .opacity(selectedTab == mode ? 1 : 0)
                    .allowsHitTesting(selectedTab == mode)
                    .accessibilityHidden(selectedTab != mode)
            }
        }
        .toolbar {
            ToolbarItemGroup(
                placement: .principal,
                content: {
                    TabPicker(
                        selectedTab: $selectedTab
                    )
                }
            )
        }
    }
}

