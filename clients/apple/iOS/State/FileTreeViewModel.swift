import SwiftUI
import SwiftWorkspace
import Combine

class FileTreeViewModel: ObservableObject {
    @Published var openFolders: Set<UUID> = Set()
    @Published var implicitlyOpenFolders: Set<UUID> = Set()
    @Published var selectedDoc: UUID? = nil
    @Published var selectedFilesState: SelectedFilesState = .unselected
    
    private var cancellables: Set<AnyCancellable> = []
    
    let filesModel: FilesViewModel
    
    init(workspaceState: WorkspaceState, filesModel: FilesViewModel) {
        self.filesModel = filesModel
        
        workspaceState.$openDoc.sink { [weak self] selectedDoc in
            self?.selectedDoc = selectedDoc
        }
        .store(in: &cancellables)
    }
    
    func openFile(_ file: File) {
        if(self.openFolders.contains(file.id)) {
            return
        }
        
        openFolders.insert(file.id)
        implicitlyOpenFolders.formUnion(self.getParents(file))
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
    
    func addFileToSelection(file: File) {
        var (implicitly, explicitly): (Set<File>, Set<File>) = switch selectedFilesState {
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
            var childrenToAdd = filesModel.childrens[file.id] ?? []
            
            while !childrenToAdd.isEmpty {
                var newChildren: [File] = []
                for child in childrenToAdd {
                    implicitly.insert(child)
                    explicitly.remove(child)
                    if child.type == .folder {
                        newChildren.append(contentsOf: filesModel.childrens[child.id] ?? [])
                    }
                }
                
                childrenToAdd = newChildren
            }
        }
        
        self.selectedFilesState = .selected(explicitly: explicitly, implicitly: implicitly)
    }
    
    func removeFileFromSelection(file: File) {
        var (implicitly, explicitly): (Set<File>, Set<File>) = switch selectedFilesState {
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
        var maybeCurrent = filesModel.idsToFiles[file.parent]
        
        if maybeCurrent?.id != maybeCurrent?.parent {
            while let current = maybeCurrent {
                if implicitly.contains(current) {
                    explicitly.remove(current)
                    implicitly.remove(current)
                    
                    let children = filesModel.childrens[current.id] ?? []
                    for child in children {
                        if child != before {
                            implicitly.insert(child)
                            explicitly.insert(child)
                        }
                    }
                    
                    let newCurrent = filesModel.idsToFiles[current.parent]
                    before = current
                    maybeCurrent = newCurrent?.id == newCurrent?.parent ? nil : newCurrent
                } else {
                    maybeCurrent = nil
                }
            }
        }
        
        if file.type == .folder {
            var childrenToRemove = filesModel.childrens[file.id] ?? []
            
            while !childrenToRemove.isEmpty {
                var newChildren: [File] = []
                
                for child in childrenToRemove {
                    if (explicitly.remove(child) == child || implicitly.remove(child) == child) && child.type == .folder {
                        newChildren.append(contentsOf: filesModel.childrens[child.id] ?? [])
                    }
                }
                
                childrenToRemove = newChildren
            }
        }
        
        self.selectedFilesState = .selected(explicitly: explicitly, implicitly: implicitly)
    }
}

enum SelectedFilesState {
    case unselected
    case selected(explicitly: Set<File>, implicitly: Set<File>)
    
    func isSelected(_ file: File) -> Bool {
        switch self {
        case .unselected:
            return false
        case .selected(explicitly: _, implicitly: let implcitly):
            return implcitly.contains(file)
        }
    }
    
    func isSelectableState() -> Bool {
        switch self {
        case .unselected:
            return false
        case .selected(explicitly: _, implicitly: _):
            return true
        }
    }
}
