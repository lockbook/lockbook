import SwiftUI
import SwiftWorkspace
import Combine

class FileTreeViewModel: ObservableObject {
    @Published var openFolders: Set<UUID> = Set()
    @Published var openDoc: UUID? = nil
    
    private var cancellables: Set<AnyCancellable> = []
    
    var supressNextOpenFolder: Bool = false
    
    let filesModel: FilesViewModel
        
    init(filesModel: FilesViewModel, workspaceInput: WorkspaceInputState, workspaceOutput: WorkspaceOutputState) {
        self.filesModel = filesModel
        
        workspaceOutput.$openDoc.sink { [weak self] openDoc in
            guard let openDoc else {
                return
            }
            
            guard let file = filesModel.idsToFiles[openDoc] else {
                return
            }
            DispatchQueue.main.async {
                workspaceInput.selectFolder(id: file.parent)
            }

            self?.openDoc = openDoc
            self?.expandToFile(file: file)
        }
        .store(in: &cancellables)
        
        workspaceOutput.$selectedFolder.sink { [weak self] selectedFolder in
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
            // go to it
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
}
