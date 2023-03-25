import Foundation
import SwiftLockbookCore

class FileService: ObservableObject {
    let core: LockbookApi

    @Published var root: File? = nil
    @Published var idsAndFiles: [UUID:File] = [:]
    var files: [File] {
        get {
            Array(idsAndFiles.values)
        }
    }
    
    var successfulAction: FileAction? = nil
        

    func childrenOf(_ meta: File?) -> [File] {
        var file: File
        if meta == nil {
            guard let theRoot = root else {
                return []
            }
            file = theRoot
        } else {
            file = meta!
        }

        var toBeSorted = files.filter {
            $0.parent == file.id && $0.parent != $0.id
        }

        toBeSorted.sort()

        return toBeSorted
    }

    func childrenOfRoot() -> [File] {
        let root = root!
        return childrenOf(root)
    }

    init(_ core: LockbookApi) {
        self.core = core

        if DI.accounts.account != nil {
            refresh()
        }
    }

    // TODO in the future we should pop one of these bad boys up during this operation
    // https://github.com/elai950/AlertToast
    func moveFile(id: UUID, newParent: UUID) {
        print("moving file")
        DispatchQueue.global(qos: .userInteractive).async {
            let operation = self.core.moveFile(id: id, newParent: newParent)

            DispatchQueue.main.async {
                switch operation {
                case .success(_):
                    self.successfulAction = .move
                    self.refresh()
                    DI.status.checkForLocalWork()
                case .failure(let error):
                    switch error.kind {
                    case .UiError(let uiError):
                        switch uiError {
                        case .FolderMovedIntoItself:
                            DI.errors.errorWithTitle("Move Error", "Cannot move a folder into itself or one of it's children")
                        case .TargetParentHasChildNamedThat:
                            DI.errors.errorWithTitle("Move Error", "Target folder has a child named that")
                        default:
                            DI.errors.handleError(error)
                        }
                    default:
                        DI.errors.handleError(error)
                    }
                }
            }
        }
    }

    func moveFileSync(id: UUID, newParent: UUID) -> Bool {
        print("moving file")
        let operation = core.moveFile(id: id, newParent: newParent)

        switch operation {
        case .success(_):
            self.successfulAction = .move
            refresh()
            DI.status.checkForLocalWork()
            return true
        case .failure(let error):
            switch error.kind {
            case .UiError(let uiError):
                switch uiError {
                case .FolderMovedIntoItself:
                    DI.errors.errorWithTitle("Move Error", "Cannot move a folder into itself or one of it's children")
                case .TargetParentHasChildNamedThat:
                    DI.errors.errorWithTitle("Move Error", "Target folder has a child named that")
                default:
                    DI.errors.handleError(error)
                }
            default:
                DI.errors.handleError(error)
            }
            return false
        }
    }

    func deleteFile(id: UUID) {
        DispatchQueue.global(qos: .userInitiated).async {
            let operation = self.core.deleteFile(id: id)

            DispatchQueue.main.async {
                switch operation {
                case .success(_):
                    if DI.documentLoader.meta?.id == id {
                        DI.documentLoader.deleted = true
                    }
                    self.successfulAction = .delete
                    self.refresh()
                    DI.status.checkForLocalWork()
                case .failure(let error):
                    DI.errors.handleError(error)
                }
            }
        }
    }

    func renameFile(id: UUID, name: String) {
        DispatchQueue.global(qos: .userInteractive).async {
            let operation = self.core.renameFile(id: id, name: name)

            DispatchQueue.main.async {
                switch operation {
                case .success(_):
                    self.successfulAction = .rename
                    self.refresh()
                    DI.status.checkForLocalWork()
                case .failure(let error):
                    switch error.kind {
                    case .UiError(let uiError):
                        switch uiError {
                        case .FileNameNotAvailable:
                            DI.errors.errorWithTitle("Rename Error", "File with that name exists already")
                        case .NewNameContainsSlash:
                            DI.errors.errorWithTitle("Rename Error", "Filename cannot contain slash")
                        case .NewNameEmpty:
                            DI.errors.errorWithTitle("Rename Error", "Filename cannot be empty")
                        default:
                            DI.errors.handleError(error)
                        }
                    default:
                        DI.errors.handleError(error)
                    }
                }
            }
        }
    }
    
    func filesToExpand(pathToRoot: [File], currentFile: File) -> [File] {
        if(currentFile.isRoot) {
            return []
        }
        
        let parentFile = idsAndFiles[currentFile.parent]!
        
        var pathToRoot = filesToExpand(pathToRoot: pathToRoot, currentFile: parentFile)
        
        if(currentFile.fileType == .Folder) {
            pathToRoot.append(currentFile)
        }
        
        return pathToRoot
    }

    func refresh() {
        DispatchQueue.global(qos: .userInteractive).async {
            let allFiles = self.core.listFiles()

            DispatchQueue.main.async {
                switch allFiles {
                case .success(let files):
                    self.idsAndFiles = Dictionary(uniqueKeysWithValues: files.map { ($0.id, $0) })
                    self.files.forEach {
                        self.notifyDocumentChanged($0)
                        if self.root == nil && $0.id == $0.parent {
                            self.root = $0
                        }
                    }
                    self.closeOpenFileIfDeleted()
                case .failure(let error):
                    DI.errors.handleError(error)
                }
            }
        }
    }

    private func closeOpenFileIfDeleted() {
        if let id = DI.documentLoader.meta?.id {
            if !files.contains(where: { $0.id == id }) {
                DI.documentLoader.deleted = true
            }
        }
    }

    private func notifyDocumentChanged(_ meta: File) {
        if let openDocument = DI.documentLoader.meta, meta.id == openDocument.id, meta.lastModified != openDocument.lastModified {
            DI.documentLoader.updatesFromCoreAvailable(meta)
        }
    }
}

public enum FileAction {
    case move
    case rename
    case delete
    case createFolder
}
