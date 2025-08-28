import SwiftUI
import SwiftWorkspace
import Combine

class HomeState: ObservableObject {
    @Published var fileActionCompleted: FileAction? = nil
    
    @Published var showSettings: Bool = false
    @Published var showPendingShares: Bool = false
    @Published var showUpgradeAccount: Bool = false
    
    @Published var sheetInfo: FileOperationSheetInfo? = nil
    @Published var selectSheetInfo: SelectFolderAction? = nil
    @Published var tabsSheetInfo: TabSheetInfo? = nil
    
    @Published var constrainedSidebarState: ConstrainedSidebarState = .closed
    @Published var showTabsSheet: Bool = false
    @Published var showOutOfSpaceAlert: Bool = false
    
    init() {
        #if os(iOS)
        expandSidebarIfNoDocs()
        #endif
    }
    
    func expandSidebarIfNoDocs() {
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) {
            self.constrainedSidebarState = .openPartial
        }
    }
    
    func closeWorkspaceBlockingScreens() {
        showSettings = false
        showPendingShares = false
        showUpgradeAccount = false
    }
}

public enum ConstrainedSidebarState {
    case closed
    case openPartial
}

public enum FileAction {
    case move
    case delete
    case createFolder
    case importFiles
    case acceptedShare
}

struct TabSheetInfo: Identifiable {
    let id = UUID()
    
    let info: [(name: String, id: UUID)]
}

enum UsageBarDisplayMode: String, Codable, CaseIterable, Identifiable {
    case always
    case never
    case whenHalf
    
    var id: Self { self }
    
    var label: String {
        switch self {
        case .always: "Always show"
        case .never: "Never show"
        case .whenHalf: "Show above 50%"
        }
    }
}
