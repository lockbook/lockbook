import SwiftUI

struct MobileCustomTabView<TabContent: View>: View {
    @Binding var selectedTab: TabType
    @ViewBuilder var tabContent: (TabType) -> TabContent
    
    @EnvironmentObject var filesModel: FilesViewModel

    var body: some View {
        TabView {
            ForEach(TabType.allCases) { mode in
                Tab(
                    mode.title,
                    systemImage: mode.systemImage
                ) {
                    tabContent(mode)
                }.badge(badgeCount(mode))
            }
        }
    }
    
    func badgeCount(_ mode: TabType) -> Int {
        switch mode {
        case .home:
            0
        case .sharedWithMe:
            filesModel.pendingSharesByUsername?.values.reduce(0) { $0 + $1.count } ?? 0
        }
    }
}
