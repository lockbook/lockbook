import SwiftUI
import SwiftWorkspace
import Combine

class FilesViewModel: ObservableObject {
    
    @Published var loaded: Bool = false
    @Published var root: File? = nil
    @Published var files: [File] = []
    var idsToFiles: [UUID: File] = [:]
    var childrens: [UUID: [File]] = [:]
    
    @Published var selectedFilesState: SelectedFilesState = .unselected
    @Published var deleteFileConfirmation: [File]? = nil
    
    var error: String? = nil
    
    private var cancellables: Set<AnyCancellable> = []
        
    init() {
        AppState.lb.events.$metadataUpdated.sink { [weak self] status in
            self?.loadFiles()
        }
        .store(in: &cancellables)
        
        self.loadFiles()
    }
    
    func isFileInDeletion(id: UUID) -> Bool {
        return deleteFileConfirmation?.count == 1 && deleteFileConfirmation?[0].id == id
    }
    
    func isMoreThanOneFileInDeletion() -> Bool {
        return deleteFileConfirmation?.count ?? 0 > 1
    }
    
    func addFileToSelection(file: File) {
        var (explicitly, implicitly): (Set<File>, Set<File>) = switch selectedFilesState {
        case .unselected:
            ([], [])
        case .selected(explicitly: let explicitly, implicitly: let implicitly):
            (explicitly, implicitly)
        }
        
        if implicitly.contains(file) {
            return
        }
        
        explicitly.insert(file)
        implicitly.insert(file)
                
        if file.type == .folder {
            var childrenToAdd = self.childrens[file.id] ?? []
            
            while !childrenToAdd.isEmpty {
                var newChildren: [File] = []
                for child in childrenToAdd {
                    implicitly.insert(child)
                    explicitly.remove(child)
                    if child.type == .folder {
                        newChildren.append(contentsOf: self.childrens[child.id] ?? [])
                    }
                }
                
                childrenToAdd = newChildren
            }
        }
        
        self.selectedFilesState = .selected(explicitly: explicitly, implicitly: implicitly)
    }
    
    func removeFileFromSelection(file: File) {
        var (explicitly, implicitly): (Set<File>, Set<File>) = switch selectedFilesState {
        case .unselected:
            ([], [])
        case .selected(explicitly: let explicitly, implicitly: let implicitly):
            (explicitly, implicitly)
        }
        
        if !implicitly.contains(file) {
            return
        }
    
        explicitly.remove(file)
        implicitly.remove(file)
        
        var before = file
        var maybeCurrent = self.idsToFiles[file.parent]
        
        if maybeCurrent?.id != maybeCurrent?.parent {
            while let current = maybeCurrent {
                if implicitly.contains(current) {
                    explicitly.remove(current)
                    implicitly.remove(current)
                    
                    let children = self.childrens[current.id] ?? []
                    for child in children {
                        if child != before {
                            implicitly.insert(child)
                            explicitly.insert(child)
                        }
                    }
                    
                    let newCurrent = self.idsToFiles[current.parent]
                    before = current
                    maybeCurrent = newCurrent?.id == newCurrent?.parent ? nil : newCurrent
                } else {
                    maybeCurrent = nil
                }
            }
        }
        
        if file.type == .folder {
            var childrenToRemove = self.childrens[file.id] ?? []
            
            while !childrenToRemove.isEmpty {
                var newChildren: [File] = []
                
                for child in childrenToRemove {
                    if (explicitly.remove(child) == child || implicitly.remove(child) == child) && child.type == .folder {
                        newChildren.append(contentsOf: self.childrens[child.id] ?? [])
                    }
                }
                
                childrenToRemove = newChildren
            }
        }
        
        self.selectedFilesState = .selected(explicitly: explicitly, implicitly: implicitly)
    }
    
    func getConsolidatedSelection() -> [File] {
        var selected: [File] = []
        let explicitly: Set<File> = switch selectedFilesState {
        case .unselected:
            []
        case .selected(explicitly: let explicitly, implicitly: _):
            explicitly
        }
        
        
        for file in explicitly {
            var isUniq = true
            var parent = self.idsToFiles[file.parent]
            
            while let newParent = parent, !newParent.isRoot {
                if explicitly.contains(newParent) == true {
                    isUniq = false
                    break
                }
                
                parent = self.idsToFiles[newParent.parent]
            }
            
            if isUniq {
                selected.append(file)
            }
        }
        
        return selected
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
                            self.childrens[file.parent]!.append(file) // Maybe just do binary insert
                            self.childrens[file.parent]!.sort {
                                if $0.type == $1.type {
                                    return $0.name < $1.name
                                } else {
                                    return $0.type == .folder
                                }
                            }
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
                let name = "untitled\(attempt != 0 ? "-\(attempt)" : "")\(ext)"

                switch AppState.lb.createFile(name: name, parent: parent, fileType: .document) {
                case .success(let file):
                    created = file
                    AppState.workspaceState.requestOpenDoc(file.id)
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
    
    func deleteFiles(files: [File]) {
        for file in files {
            if case .failure(let err) = AppState.lb.deleteFile(id: file.id) {
                self.error = err.msg
            }
            
            AppState.workspaceState.fileOpCompleted = .Delete(id: file.id)
        }
        
        self.loadFiles()
        self.selectedFilesState = .unselected
    }
}
