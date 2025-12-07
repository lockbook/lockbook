import SwiftUI


struct TabbedSplitView<Sidebar: View, Detail: View>: View {
    @State private var selectedTab: Tab
    
    // Closures to generate views based on the tab
    let sidebar: (Tab) -> Sidebar
    @ViewBuilder let detail: () -> Detail
    
    @Environment(\.horizontalSizeClass) var horizontalSizeClass
    
    init(initialTab: Tab,
         @ViewBuilder sidebar: @escaping (Tab) -> Sidebar,
         @ViewBuilder detail: @escaping () -> Detail) {
        self._selectedTab = State(initialValue: initialTab)
        self.sidebar = sidebar
        self.detail = detail
    }
    
    var body: some View {
        #if os(macOS)
        splitView
        #elseif os(iOS)
        if horizontalSizeClass == .compact {
            bottomTabView
        } else {
            splitView
        }
        #else
        bottomTabView
        #endif
    }
    
    var splitView: some View {
        NavigationSplitView {
            // The sidebar's content is simply the generic sidebar view based on selection
            sidebar(selectedTab)
                .navigationTitle(selectedTab.title)
                .toolbarRole(.navigationStack) // Prevents toolbar promotion when collapsed
                .toolbar {
                    // Place the modular rail view in the toolbar
                    ToolbarItemGroup(placement: .navigationBarLeading) {
                        VerticalTabRailView(selectedTab: $selectedTab)
                    }
                }
        } detail: {
            detail() // The generic Detail view
        }
    }
    
    var bottomTabView: some View {
        TabView(selection: $selectedTab) {
            ForEach(Tab.allCases, id: \.self) { tabCase in
                sidebar(tabCase) // Content closure passed from initializer
                    .tabItem {
                        Label(tabCase.title, systemImage: tabCase.systemImage)
                    }
                    .tag(tabCase)
            }
        }
    }
    
}

struct PickerTabRailView: View {
    @Binding var selectedTab: Tab

    var body: some View {
        Picker("Tabs", selection: $selectedTab) {
            ForEach(Tab.allCases) { mode in
                Label(mode.title, systemImage: mode.systemImage)
                    .tag(mode)
            }
        }
        .pickerStyle(.segmented)
        .frame(maxWidth: 160)
    }
}

enum Tab: Hashable, CaseIterable, Identifiable {
    case home
    case sharedWithMe

    var id: Self { self }
    
    var title: String {
        switch self {
        case .home: return "Home"
        case .sharedWithMe: return "Shared"
        }
    }
    
    var systemImage: String {
        switch self {
        case .home: return "house"
        case .sharedWithMe: return "person.2.fill"
        }
    }
}
