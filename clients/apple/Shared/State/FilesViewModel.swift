import Combine
import SwiftUI
import SwiftWorkspace

class FilesViewModel: ObservableObject {

    @Published var root: File? = nil
    @Published var files: [File] = []
    var idsToFiles: [UUID: File] = [:]
    var childrens: [UUID: [File]] = [:]
    var pendingSharesAndChildren: [UUID] = []
    
    @Published var pendingSharesByUsername: [String: [File]]? = nil

    @Published var selectedFilesState: SelectedFilesState = .unselected
    @Published var deleteFileConfirmation: [File]? = nil

    var error: String? = nil

    private var cancellables: Set<AnyCancellable> = []

    init() {
        AppState.lb.events.$metadataUpdated.sink { [weak self] status in
            self?.loadFiles()
        }
        .store(in: &cancellables)
    }

    func isFileInDeletion(id: UUID) -> Bool {
        return deleteFileConfirmation?.count == 1
            && deleteFileConfirmation?[0].id == id
    }

    func isMoreThanOneFileInDeletion() -> Bool {
        return deleteFileConfirmation?.count ?? 0 > 1
    }

    func addFileToSelection(file: File) {
        var (explicitly, implicitly): (Set<File>, Set<File>) =
            switch selectedFilesState {
            case .unselected:
                ([], [])
            case .selected(let explicitly, let implicitly):
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
                        newChildren.append(
                            contentsOf: self.childrens[child.id] ?? []
                        )
                    }
                }

                childrenToAdd = newChildren
            }
        }

        self.selectedFilesState = .selected(
            explicitly: explicitly,
            implicitly: implicitly
        )
    }

    func removeFileFromSelection(file: File) {
        var (explicitly, implicitly): (Set<File>, Set<File>) =
            switch selectedFilesState {
            case .unselected:
                ([], [])
            case .selected(let explicitly, let implicitly):
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
                    maybeCurrent =
                        newCurrent?.id == newCurrent?.parent ? nil : newCurrent
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
                    if (explicitly.remove(child) == child
                        || implicitly.remove(child) == child)
                        && child.type == .folder
                    {
                        newChildren.append(
                            contentsOf: self.childrens[child.id] ?? []
                        )
                    }
                }

                childrenToRemove = newChildren
            }
        }

        self.selectedFilesState = .selected(
            explicitly: explicitly,
            implicitly: implicitly
        )
    }

    func getConsolidatedSelection() -> [File] {
        var selected: [File] = []
        let explicitly: Set<File> =
            switch selectedFilesState {
            case .unselected:
                []
            case .selected(let explicitly, implicitly: _):
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
        // Load files
        DispatchQueue.global(qos: .userInitiated).async {
            let res = AppState.lb.listMetadatas()

            DispatchQueue.main.sync {
                switch res {
                case .success(let files):
                    self.idsToFiles = [:]
                    self.childrens = [:]

                    files.forEach { file in
                        self.insertIntoFiles(file: file)
                    }
                    self.files = files
                case .failure(let err):
                    self.error = err.msg
                }
            }
            
            guard let root = self.root else {
                AppState.shared.error = .custom(title: "Unexpected error", msg: "Could not find root")
                
                return
            }
            
            // Load all pending shares
            let pendingShareFilesRes = AppState.lb.getPendingShareFiles()

            DispatchQueue.main.sync {
                switch pendingShareFilesRes {
                case .success(let files):
                    for file in files {
                        self.insertIntoFiles(file: file)
                    }
                    
                    self.pendingSharesAndChildren = files.map({ $0.id })
                case .failure(let err):
                    self.error = err.msg
                }
            }
            print("inserted these pending shares", self.childrens[UUID(uuidString: "FA41D0AA-2491-4BEE-B5BB-3E3C8C31944D")!])


            // Load pending shares
            let pendingSharesRes = AppState.lb.getPendingShares()

            DispatchQueue.main.sync {
                switch pendingSharesRes {
                case .success(let files):
                    var pendingShares: [String: [File]] = [:]
                    
                    for file in files {
                        guard let sharedBy = file.shareFrom(to: root.name) else {
                            continue
                        }
                        
                        if pendingShares[sharedBy] == nil {
                            pendingShares[sharedBy] = []
                        }
                        
                        pendingShares[sharedBy]!.append(file)
                    }
                    
                    self.pendingSharesByUsername = pendingShares
                case .failure(let err):
                    self.error = err.msg
                }

            }
        }
    }

    private func insertIntoFiles(file: File) {
        self.idsToFiles[file.id] = file

        if self.childrens[file.parent] == nil {
            self.childrens[file.parent] = []
        }

        if !file.isRoot {
            if !(self.childrens[file.parent] ?? []).contains(file) {
                self.childrens[file.parent]!.append(file)  // Maybe just do binary insert
                self.childrens[file.parent]!.sort {
                    if $0.type == $1.type {
                        return $0.name < $1.name
                    } else {
                        return $0.type == .folder
                    }
                }
            }
        } else if self.root == nil {
            self.root = file
        }
    }

    func deleteFiles(files: [File], workspaceInput: WorkspaceInputState) {
        for file in files {
            if case .failure(let err) = AppState.lb.deleteFile(id: file.id) {
                self.error = err.msg
            }

            workspaceInput.fileOpCompleted(fileOp: .Delete(id: file.id))
        }

        self.loadFiles()
        self.selectedFilesState = .unselected
    }
    
    func rejectShare(id: UUID) {
        if case let .failure(err) = AppState.lb.deletePendingShare(id: id) {
            AppState.shared.error = .lb(error: err)
        }
    }

}

extension FilesViewModel {
    static var preview: FilesViewModel {
        return FilesViewModel()
    }
}
