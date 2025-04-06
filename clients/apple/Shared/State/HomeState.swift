import SwiftUI
import SwiftWorkspace
import Combine

class HomeState: ObservableObject {
    private var cancellables: Set<AnyCancellable> = []
    
    init() {
        AppState.workspaceState.$renameOpenDoc.sink { [weak self] rename in
            self?.runOnActiveWorkspaceState(doRun: rename) { file in
                self?.sheetInfo = .rename(file: file)
            }
        }
        .store(in: &cancellables)
        
        AppState.workspaceState.$newFolderButtonPressed.sink { [weak self] newFolder in
            guard newFolder else {
                return
            }
            
            guard let root = try? AppState.lb.getRoot().get() else {
                return
            }
            
            self?.sheetInfo = .createFolder(parent: root)
        }
        .store(in: &cancellables)
    }
    
    func runOnActiveWorkspaceState(doRun: Bool, f: (File) -> Void) {
        guard let openDoc = AppState.workspaceState.openDoc else {
            return
        }
        
        if doRun {
            if let file = try? AppState.lb.getFile(id: openDoc).get() {
                f(file)
            }
        }
    }
    
    @Published var error: UIError? = nil
    @Published var fileActionCompleted: FileAction? = nil
    
    @Published var showSettings: Bool = false
    @Published var showPendingShares: Bool = false
    
    @Published var sheetInfo: FileOperationSheetInfo? = nil
    @Published var selectSheetInfo: SelectFolderAction? = nil
    @Published var tabsSheetInfo: TabSheetInfo? = nil
    
    @Published var constrainedSidebarState: ConstrainedSidebarState = .openPartial
    @Published var showTabsSheet: Bool = false
}

public enum ConstrainedSidebarState {
    case closed
    case openPartial
    case openExpanded
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
