import SwiftUI


struct TabPicker: View {
    @Binding var selectedTab: TabType

    var body: some View {
        Picker("Tabs", selection: $selectedTab) {
            ForEach(TabType.allCases) { mode in
                Label(mode.title, systemImage: mode.systemImage)
                    .tag(mode)
            }
        }
        .pickerStyle(.segmented)
        .fixedSize()
    }
}

enum TabType: Hashable, CaseIterable, Identifiable {
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
