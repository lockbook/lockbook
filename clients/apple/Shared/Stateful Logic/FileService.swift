import Foundation
import SwiftUI
import SwiftLockbookCore

class FileService: ObservableObject {
    let core: LockbookApi

    @Published var root: File? = nil
    @Published var idsAndFiles: [UUID:File] = [:]
    @Published var suggestedDocs: [File]? = nil
    var files: [File] {
        get {
            Array(idsAndFiles.values)
        }
    }
    @Published var successfulAction: FileAction? = nil

    // File Service keeps track of the parent being displayed on iOS. Since this functionality is not used for macOS, it is conditionally compiled.
#if os(iOS)
    @Published var path: [File] = []

    var parent: File? {
        get {
            path.last
        }
    }

    func childrenOfParent() -> [File] {
        return childrenOf(path.last)
    }

    func upADirectory() {
        self.path.removeLast()
    }

    func intoChildDirectory(_ file: File) {
        self.path.append(file)
    }

    func pathBreadcrumbClicked(_ file: File) {
        if let firstIndex = self.path.firstIndex(of: file) {
            self.path.removeSubrange(firstIndex + 1...self.path.count - 1)
        }
    }
#endif

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
                    self.refresh()
                    self.successfulAction = .delete
                    DI.status.checkForLocalWork()
                case .failure(let error):
                    DI.errors.handleError(error)
                }
            }
        }
    }

    func renameFileSync(id: UUID, name: String) -> String? {
        let operation = self.core.renameFile(id: id, name: name)

        switch operation {
        case .success(_):
            #if os(macOS)
            idsAndFiles[id]?.name = name
            #endif
            return nil
        case .failure(let error):
            switch error.kind {
            case .UiError(let uiError):
                switch uiError {
                case .FileNameNotAvailable:
                    return "A file with that name already exists"
                case .NewNameContainsSlash:
                    return "Your filename cannot contain a slash"
                case .NewNameEmpty:
                    return "Your filename cannot be empty"
                case .FileNameTooLong:
                    return "Your filename is too long"
                default:
                    return "An error occurred while renaming the file"
                }
            default:
                return "An error occurred while renaming the file"
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
    
    func refreshSuggestedDocs() {
        DispatchQueue.global(qos: .userInitiated).async {
            switch self.core.suggestedDocs() {
            case .success(let ids):
                var suggestedDocs: [File] = []
                    
                for id in ids.filter({ self.idsAndFiles[$0] != nil }) {
                    switch self.core.getFileById(id: id) {
                    case .success(let meta):
                        suggestedDocs.append(meta)
                    case .failure(let error):
                        if error.kind != .UiError(.NoFileWithThatId) {
                            DI.errors.handleError(error)
                        }
                    }
                }
                    
                DispatchQueue.main.async {
                    self.suggestedDocs = suggestedDocs
                }
            case .failure(let error):
                DI.errors.handleError(error)
            }
        }
    }

    func refresh() {
        DispatchQueue.global(qos: .userInteractive).async {
            let allFiles = self.core.listFiles()

            DispatchQueue.main.async {
                switch allFiles {
                case .success(let files):
                    self.postRefreshFiles(files)
                case .failure(let error):
                    DI.errors.handleError(error)
                }
            }
        }
    }
    
    func refreshSync() {
        let allFiles = self.core.listFiles()

        switch allFiles {
        case .success(let files):
            postRefreshFiles(files)
        case .failure(let error):
            DI.errors.handleError(error)
        }
    }
    
    private func postRefreshFiles(_ newFiles: [File]) {
        idsAndFiles = Dictionary(uniqueKeysWithValues: newFiles.map { ($0.id, $0) })
        refreshSuggestedDocs()
        newFiles.forEach {
            if root == nil && $0.id == $0.parent {
                root = $0

                #if os(iOS)
                if(path.isEmpty) {
                    path.append($0)
                }
                #endif
            }
        }
        openFileChecks()
    }

    private func openFileChecks() {
        for docInfo in DI.currentDoc.openDocuments.values {
            let maybeMeta = idsAndFiles[docInfo.meta.id]
            
            if let meta = maybeMeta {
                if (meta.lastModified != docInfo.meta.lastModified) || (meta != docInfo.meta) {
                    docInfo.updatesFromCoreAvailable(meta)
                }
            } else {
                docInfo.deleted = true
            }
        }
        
        if let selectedFolder = DI.currentDoc.selectedFolder {
            let maybeMeta = idsAndFiles[selectedFolder.id]
            
            if maybeMeta == nil {
                DI.currentDoc.selectedFolder = nil
            }
        }
    }

    public func createDoc(maybeParent: UUID? = nil, isDrawing: Bool) {
        DispatchQueue.global(qos: .userInitiated).async {
            let realParent = maybeParent ?? {
#if os(iOS)
                self.parent?.id ?? self.root!.id
#else
                DI.currentDoc.selectedFolder?.id ?? self.root!.id
#endif
            }()
            
            var name = ""
            let fileExt = isDrawing ? ".draw" : ".md"
            let namePart = isDrawing ? "untitled-drawing-" : "untitled-doc-"
            var attempt = 0
            
            while(true) {
                name = namePart + String(attempt)
                
                switch self.core.createFile(name: name + fileExt, dirId: realParent, isFolder: false) {
                case .success(let meta):
                    self.refreshSync()
                    
                    DispatchQueue.main.async {
                        DI.currentDoc.cleanupOldDocs()
                        DI.currentDoc.justCreatedDoc = self.idsAndFiles[meta.id]
                        DI.currentDoc.openDoc(id: meta.id)
                        DI.currentDoc.setSelectedOpenDocById(maybeId: meta.id)
                    }
                    
                    return
                case .failure(let err):
                    switch err.kind {
                    case .UiError(.FileNameNotAvailable):
                        attempt += 1
                        continue
                    default:
                        DI.errors.handleError(err)
                        return
                    }
                }
            }
        }
    }
    
    public func createFolderSync(name: String, maybeParent: UUID? = nil) -> String? {
        let realParent = maybeParent ?? {
            #if os(iOS)
            parent?.id ?? root!.id
            #else
            DI.currentDoc.selectedFolder?.id ?? root!.id
            #endif
        }()
        
        switch core.createFile(name: name, dirId: realParent, isFolder: true) {
        case .success(_):
            refresh()
            return nil
        case .failure(let err):
            switch err.kind {
            case .UiError(.FileNameContainsSlash):
                return "Your file name contains a slash"
            case .UiError(.FileNameEmpty):
                return "Your file name cannot be empty"
            case .UiError(.FileNameNotAvailable):
                return "Your file name is not available"
            case .UiError(.FileNameTooLong):
                return "Your file name is too long"
            default:
                return "An error has occurred"
            }
        }
    }
    
    public func getPathByIdOrParent(maybeId: UUID? = nil) -> String? {
        let id = maybeId ?? {
            #if os(iOS)
            parent?.id ?? root!.id
            #else
            DI.currentDoc.selectedFolder?.id ?? root!.id
            #endif
        }()
        
        switch core.getPathById(id: id) {
        case .success(let path):
            return path
        case .failure(let err):
            DI.errors.handleError(err)
            return nil
        }
    }
    
    public func copyFileLink(id: UUID) {
        #if os(iOS)
        UIPasteboard.general.string = "lb://\(id.uuidString.lowercased())"
        #else
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString("lb://\(id.uuidString.lowercased())", forType: .string)
        #endif
    }
}

public enum FileAction {
    case move
    case delete
    case createFolder
    case importFiles
}
