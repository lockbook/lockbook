import SwiftUI
import SwiftWorkspace

class HomeState: ObservableObject {
    @Published var error: UIError? = nil
    @Published var fileActionCompleted: FileAction? = nil
    
    @Published var showSettings: Bool = false
    @Published var showPendingShares: Bool = false
    
    @Published var sheetInfo: FileOperationSheetInfo? = nil
    @Published var selectSheetInfo: SelectFolderAction? = nil
    @Published var tabsSheetInfo: TabSheetInfo? = nil
    
    @Published var isConstrainedSidebarOpen: Bool = true
    @Published var showTabsSheet: Bool = false
}

public enum FileAction {
    case move
    case delete
    case createFolder
    case importFiles
    case acceptedShare
}

enum UIError {
    case lb(error: LbError)
    case custom(title: String, msg: String)
}

struct TabSheetInfo: Identifiable {
    let id = UUID()
    
    let info: [(name: String, id: UUID)]
}
