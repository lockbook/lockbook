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

    var error: String?

    private var cancellables: Set<AnyCancellable> = []

    init() {
        AppState.lb.events.$metadataUpdated.sink { [weak self] _ in
            self?.loadFiles()
        }
        .store(in: &cancellables)
    }

    func isFileInDeletion(id: UUID) -> Bool {
        deleteFileConfirmation?.count == 1
            && deleteFileConfirmation?[0].id == id
    }

    func isMoreThanOneFileInDeletion() -> Bool {
        deleteFileConfirmation?.count ?? 0 > 1
    }

    func addFileToSelection(file: File) {
        var (explicitly, implicitly): (Set<File>, Set<File>) =
            switch selectedFilesState {
            case .unselected:
                ([], [])
            case let .selected(explicitly, implicitly):
                (explicitly, implicitly)
            }

        if implicitly.contains(file) {
            return
        }

        explicitly.insert(file)
        implicitly.insert(file)

        if file.type == .folder {
            var childrenToAdd = childrens[file.id] ?? []

            while !childrenToAdd.isEmpty {
                var newChildren: [File] = []
                for child in childrenToAdd {
                    implicitly.insert(child)
                    explicitly.remove(child)
                    if child.type == .folder {
                        newChildren.append(
                            contentsOf: childrens[child.id] ?? []
                        )
                    }
                }

                childrenToAdd = newChildren
            }
        }

        selectedFilesState = .selected(
            explicitly: explicitly,
            implicitly: implicitly
        )
    }

    func removeFileFromSelection(file: File) {
        var (explicitly, implicitly): (Set<File>, Set<File>) =
            switch selectedFilesState {
            case .unselected:
                ([], [])
            case let .selected(explicitly, implicitly):
                (explicitly, implicitly)
            }

        if !implicitly.contains(file) {
            return
        }

        explicitly.remove(file)
        implicitly.remove(file)

        var before = file
        var maybeCurrent = idsToFiles[file.parent]

        if maybeCurrent?.id != maybeCurrent?.parent {
            while let current = maybeCurrent {
                if implicitly.contains(current) {
                    explicitly.remove(current)
                    implicitly.remove(current)

                    let children = childrens[current.id] ?? []
                    for child in children {
                        if child != before {
                            implicitly.insert(child)
                            explicitly.insert(child)
                        }
                    }

                    let newCurrent = idsToFiles[current.parent]
                    before = current
                    maybeCurrent =
                        newCurrent?.id == newCurrent?.parent ? nil : newCurrent
                } else {
                    maybeCurrent = nil
                }
            }
        }

        if file.type == .folder {
            var childrenToRemove = childrens[file.id] ?? []

            while !childrenToRemove.isEmpty {
                var newChildren: [File] = []

                for child in childrenToRemove {
                    if explicitly.remove(child) == child
                        || implicitly.remove(child) == child,
                        child.type == .folder
                    {
                        newChildren.append(
                            contentsOf: childrens[child.id] ?? []
                        )
                    }
                }

                childrenToRemove = newChildren
            }
        }

        selectedFilesState = .selected(
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
            var parent = idsToFiles[file.parent]

            while let newParent = parent, !newParent.isRoot {
                if explicitly.contains(newParent) == true {
                    isUniq = false
                    break
                }

                parent = idsToFiles[newParent.parent]
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
                case let .success(files):
                    self.idsToFiles = [:]
                    self.childrens = [:]

                    for file in files {
                        self.insertIntoFiles(file: file)
                    }
                    self.files = files
                case let .failure(err):
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
                case let .success(files):
                    for file in files {
                        self.insertIntoFiles(file: file)
                    }

                    self.pendingSharesAndChildren = files.map(\.id)
                case let .failure(err):
                    self.error = err.msg
                }
            }
            print("inserted these pending shares", self.childrens[UUID(uuidString: "FA41D0AA-2491-4BEE-B5BB-3E3C8C31944D")!])

            // Load pending shares
            let pendingSharesRes = AppState.lb.getPendingShares()

            DispatchQueue.main.sync {
                switch pendingSharesRes {
                case let .success(files):
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
                case let .failure(err):
                    self.error = err.msg
                }
            }
        }
    }

    private func insertIntoFiles(file: File) {
        idsToFiles[file.id] = file

        if childrens[file.parent] == nil {
            childrens[file.parent] = []
        }

        if !file.isRoot {
            if !(childrens[file.parent] ?? []).contains(file) {
                childrens[file.parent]!.append(file) // Maybe just do binary insert
                childrens[file.parent]!.sort {
                    if $0.type == $1.type {
                        $0.name < $1.name
                    } else {
                        $0.type == .folder
                    }
                }
            }
        } else if root == nil {
            root = file
        }
    }

    func deleteFiles(files: [File], workspaceInput: WorkspaceInputState) {
        for file in files {
            if case let .failure(err) = AppState.lb.deleteFile(id: file.id) {
                error = err.msg
            }

            workspaceInput.fileOpCompleted(fileOp: .Delete(id: file.id))
        }

        loadFiles()
        selectedFilesState = .unselected
    }

    func rejectShare(id: UUID) {
        if case let .failure(err) = AppState.lb.deletePendingShare(id: id) {
            AppState.shared.error = .lb(error: err)
        }
    }
}

extension FilesViewModel {
    static var preview: FilesViewModel {
        FilesViewModel()
    }
}
