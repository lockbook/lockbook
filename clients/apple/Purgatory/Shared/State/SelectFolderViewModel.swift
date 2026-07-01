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
        if let folderPaths {
            if searchInput.isEmpty {
                folderPaths
            } else {
                folderPaths.filter { path in
                    path.localizedCaseInsensitiveContains(searchInput)
                }
            }
        } else {
            []
        }
    }

    @Published var selected = 0
    var selectedPath: String {
        if filteredFolderPaths.count <= selected {
            return ""
        }

        return filteredFolderPaths[selected].isEmpty ? "/" : filteredFolderPaths[selected]
    }

    var exit: Bool = false

    func calculateFolderPaths() {
        DispatchQueue.global(qos: .userInitiated).async {
            let res = AppState.lb.listFolderPaths()

            DispatchQueue.main.async {
                switch res {
                case let .success(paths):
                    self.folderPaths = paths.map { String($0.dropFirst()) }.sorted()

                case .failure:
                    self.error = "Could not get folder paths."
                }
            }
        }
    }

    func selectFolder(action: SelectFolderAction, path: String) -> Bool {
        switch AppState.lb.getByPath(path: path) {
        case let .success(parent):
            return selectFolder(action: action, parent: parent.id)
        case let .failure(err):
            error = err.msg

            return false
        }
    }

    func selectFolder(action: SelectFolderAction, parent: UUID) -> Bool {
        switch action {
        case let .move(files):
            for file in files {
                if case let .failure(err) = AppState.lb.moveFile(id: file.id, newParent: parent) {
                    error = err.msg

                    return false
                }
            }

            homeState.fileActionCompleted = .move
            filesModel.loadFiles()
            filesModel.selectedFilesState = .unselected

            return true
        case let .externalImport(urls):
            let paths = urls.map { $0.path(percentEncoded: false) }
            if case let .failure(err) = AppState.lb.importFiles(sources: paths, dest: parent) {
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
        case let .acceptShare(name, id):
            if case let .failure(err) = AppState.lb.createLink(name: name, parent: parent, target: id) {
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

extension SelectFolderViewModel {
    static var preview: SelectFolderViewModel {
        SelectFolderViewModel(homeState: .preview, filesModel: .preview)
    }
}
