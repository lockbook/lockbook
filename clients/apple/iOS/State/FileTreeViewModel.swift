import SwiftUI
import SwiftWorkspace
import Combine

class FileTreeViewModel: ObservableObject {
    @Published var openFolders: Set<UUID> = Set()
    @Published var openDoc: UUID? = nil
    
    private var cancellables: Set<AnyCancellable> = []
    
    var supressNextOpenFolder: Bool = false
    
    let filesModel: FilesViewModel
        
    init(filesModel: FilesViewModel) {
        self.filesModel = filesModel
        
        AppState.workspaceState.$openDoc.sink { [weak self] openDoc in
            guard let openDoc else {
                return
            }
            
            guard let file = filesModel.idsToFiles[openDoc] else {
                return
            }
            
            self?.openDoc = openDoc
            self?.expandToFile(file: file)
        }
        .store(in: &cancellables)
        
        AppState.workspaceState.$selectedFolder.sink { [weak self] selectedFolder in
            if self?.supressNextOpenFolder == true {
                self?.supressNextOpenFolder = false
                return
            }
            
            guard let selectedFolder else {
                return
            }
            
            guard let file = filesModel.idsToFiles[selectedFolder] else {
                return
            }
            
            print("expanding to folder... \(file.name)")
            self?.expandToFile(file: file)
        }
        .store(in: &cancellables)
    }
    
    func toggleFolder(_ id: UUID) {
        if self.openFolders.remove(id) == id {
            return
        }
        
        openFolders.insert(id)
    }
    
    func expandToFile(file: File) {
        if file.isRoot {
            return
        }
        
        if let parent = filesModel.idsToFiles[file.parent] {
            expandToFile(file: parent)
        }
        
        print("opening \(file.name)")
        openFolders.insert(file.id)
    }
        
    func getParents(_ file: File) -> [UUID] {
        var parents: [UUID] = []
        
        guard case .success(var current) = AppState.lb.getFile(id: file.parent) else {
            return []
        }
        
        while current.id != current.parent {
            parents.append(current.id)
            
            if case let .success(newCurrent) = AppState.lb.getFile(id: current.parent) {
                current = newCurrent
            } else {
                return parents
            }
        }
        
        return parents
    }
}
