import Foundation

class SelectFolderViewModel: ObservableObject {
    @Published var searchInput: String = ""
    @Published var error: String? = nil
    @Published var folderPaths: [String]? = nil
    
    let homeState: HomeState
    let filesModel: FilesViewModel
    
    init(homeState: HomeState, filesModel: FilesViewModel) {
        self.homeState = homeState
        self.filesModel = filesModel
    }
    
    var filteredFolderPaths: [String] {
        if let folderPaths = folderPaths {
            if searchInput.isEmpty {
                return folderPaths
            } else {
                return folderPaths.filter { path in
                    path.localizedCaseInsensitiveContains(searchInput)
                }
            }
        } else {
            return []
        }
    }
    
    @Published var selected = 0
    var selectedPath: String {
        get {
            if filteredFolderPaths.count <= selected {
                return ""
            }
            
            return filteredFolderPaths[selected].isEmpty ? "/" : filteredFolderPaths[selected]
        }
    }
    
    var exit: Bool = false
    
    func calculateFolderPaths() {
        DispatchQueue.global(qos: .userInitiated).async {
            let res = AppState.lb.listFolderPaths()
            
            DispatchQueue.main.async {
                switch res {
                case .success(let paths):
                    self.folderPaths = paths.map({ String($0.dropFirst()) }).sorted()
                    
                case .failure(_):
                    self.error = "Could not get folder paths."
                }
            }
        }
    }
    
    func selectFolder(action: SelectFolderAction, path: String) -> Bool {
        switch AppState.lb.getByPath(path: path) {
        case .success(let parent):
            return selectFolder(action: action, parent: parent.id)
        case .failure(let err):
            error = err.msg
            
            return false
        }
    }
    
    func selectFolder(action: SelectFolderAction, parent: UUID) -> Bool {
        switch action {
        case .move(let files):
            for file in files {
                if case .failure(let err) = AppState.lb.moveFile(id: file.id, newParent: parent) {
                    error = err.msg
                    
                    return false
                }
            }
            
            homeState.fileActionCompleted = .move
            filesModel.loadFiles()
            filesModel.selectedFilesState = .unselected
            
            return true
        case .externalImport(let urls):
            let paths = urls.map({ $0.path(percentEncoded: false) })
            if case .failure(let err) = AppState.lb.importFiles(sources: paths, dest: parent) {
                error = err.msg
                
                return false
            }
            
            homeState.fileActionCompleted = .importFiles
            filesModel.loadFiles()
            filesModel.selectedFilesState = .unselected
            
            for url in urls {
                url.stopAccessingSecurityScopedResource()
            }
            
            return true
        case .acceptShare(let name, let id):
            if case .failure(let err) = AppState.lb.createLink(name: name, parent: parent, target: id) {
                error = err.msg
                return false
            }
            
            homeState.fileActionCompleted = .acceptedShare
            filesModel.loadFiles()
            filesModel.selectedFilesState = .unselected
            
            return true
        }
    }
}

enum SelectFolderMode {
    case List
    case Tree
}


#if DEBUG
extension SelectFolderViewModel {
    static var preview: SelectFolderViewModel {
        return SelectFolderViewModel(homeState: .preview, filesModel: .preview)
    }
}
#endif
