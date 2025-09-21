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
    
    @Published var sidebarState: SidebarState = .open
    @Published var isSidebarFloating: Bool = true
    
    var splitViewVisibility: Binding<NavigationSplitViewVisibility> {
        Binding(
            get: {
                switch self.sidebarState {
                case .open:
                    return .all
                case .closed:
                    return .detailOnly
                }
            },
            set: { newVisibility in
                switch newVisibility {
                case .all:
                    self.sidebarState = .open
                case .detailOnly:
                    self.sidebarState = .closed
                default:
                    break
                }
            }
        )
    }
    
    @Published var showTabsSheet: Bool = false
    @Published var showOutOfSpaceAlert: Bool = false
    
    init() {
        #if os(iOS)
        expandSidebarIfNoDocs()
        #endif
    }
    
    func expandSidebarIfNoDocs() {
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) {
            self.sidebarState = .open
        }
    }
    
    func closeWorkspaceBlockingScreens() {
        showSettings = false
        showPendingShares = false
        showUpgradeAccount = false
    }
}

public enum SidebarState {
    case closed
    case open
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
