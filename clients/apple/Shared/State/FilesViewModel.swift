import SwiftUI
import SwiftWorkspace
import Combine

class FilesViewModel: ObservableObject {
    
    @Published var loaded: Bool = false
    @Published var root: File? = nil
    @Published var files: [File] = []
    var idsToFiles: [UUID: File] = [:]
    var childrens: [UUID: [File]] = [:]
    
    var error: String? = nil
    
    private var cancellables: Set<AnyCancellable> = []
    
    let workspaceState: WorkspaceState
    
    init(workspaceState: WorkspaceState) {
        self.workspaceState = workspaceState
        workspaceState.$reloadFiles.sink { [weak self] reload in
            if reload {
                self?.loadFiles()
            }
        }
        .store(in: &cancellables)
        
        self.loadFiles()
    }
    
    func loadFiles() {
        DispatchQueue.global(qos: .userInitiated).async {
            let res = AppState.lb.listMetadatas()
            DispatchQueue.main.async {
                switch res {
                case .success(let files):
                    self.idsToFiles = [:]
                    self.childrens = [:]

                    files.forEach { file in
                        self.idsToFiles[file.id] = file
                        
                        if self.childrens[file.parent] == nil {
                            self.childrens[file.parent] = []
                        }
                        
                        if !file.isRoot {
                            self.childrens[file.parent]!.append(file)
                        } else if self.root == nil {
                            self.root = file
                        }
                    }
                    self.files = files
                    self.loaded = true
                case .failure(let err):
                    self.error = err.msg
                }
            }
        }
    }
    
    func createDoc(parent: UUID, isDrawing: Bool) {
        DispatchQueue.global(qos: .userInitiated).async {
            let ext = isDrawing ? ".svg" : ".md"
            var attempt = 0
            var created: File? = nil
            
            while created == nil {
                let name = "unititled\(attempt != 0 ? "-" : "")\(ext)"

                switch AppState.lb.createFile(name: name, parent: parent, fileType: .document) {
                case .success(let file):
                    created = file
                    self.workspaceState.requestOpenDoc(file.id)
                case .failure(let error):
                    if error.code == .pathTaken {
                        attempt += 1
                    } else {
                        return
                    }
                }
            }
            
            // Optimization: Can add the new file to our caches ourselves
            self.loadFiles()
        }
    }
    
    
}
