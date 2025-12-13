import SwiftUI

struct MobileCustomTabView<TabContent: View>: View {
    @Binding var selectedTab: TabType
    @ViewBuilder var tabContent: (TabType) -> TabContent

    var body: some View {
        TabView {
            ForEach(TabType.allCases) { mode in
                Tab(
                    mode.title,
                    systemImage: mode.systemImage
                ) {
                    tabContent(mode)
                }
            }
        }
    }
}
