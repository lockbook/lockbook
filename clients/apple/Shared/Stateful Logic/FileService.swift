import Foundation
import SwiftUI
import SwiftLockbookCore
import SwiftWorkspace
import Combine

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
    var hasRootLoaded = false

    // File Service keeps track of the parent being displayed on iOS. Since this functionality is not used for macOS, it is conditionally compiled.
    #if os(iOS)
    @Published var path: [File] = []

    var parent: File? {
        get {
            path.last
        }
    }

    func childrenOfParent() -> [File] {
        childrenOf(parent)
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
    #else
    var expandedFolders: [UUID] = []
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
    
    private var cancellables: Set<AnyCancellable> = []

    init(_ core: LockbookApi) {
        self.core = core

        if DI.accounts.account != nil {
            refresh()
        }
        
        DI.workspace.$reloadFiles.sink { reload in
            if reload {
                DI.workspace.reloadFiles = false
                
                self.refresh()
                DI.share.calculatePendingShares()
            }
        }
        .store(in: &cancellables)
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
    
    func moveFiles(ids: [UUID], newParent: UUID) -> Bool {
        var ids = Set(ids)
        
        for id in ids {
            var parent = idsAndFiles[id]?.parent
            
            while parent != nil && parent != root?.id {
                if ids.contains(parent!) {
                    ids.remove(id)
                    break
                }
                
                parent = idsAndFiles[parent!]?.parent
            }
            
        }
        
        var parent = idsAndFiles[newParent]?.parent
        
        while parent != nil && parent != root?.id {
            if ids.contains(parent!) {
                return false
            }
            
            parent = idsAndFiles[parent!]?.parent
        }
        
        for id in ids {
            let res = core.moveFile(id: id, newParent: newParent)

            if case .failure(let error) = res {
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
        
        self.successfulAction = .move
        refresh()
        return true
    }

    func deleteFiles(ids: [UUID]) {
        DispatchQueue.global(qos: .userInitiated).async {
            for id in ids {
                let res = self.core.deleteFile(id: id)
                
                if case .failure(let error) = res {
                    if error.kind != .UiError(.FileDoesNotExist) {
                        DI.errors.handleError(error)
                        return
                    }
                }
                
                DispatchQueue.main.sync {
                    DI.workspace.fileOpCompleted = .Delete(id: id)
                }
            }
            
            
            self.refresh()
            DispatchQueue.main.sync {
                self.successfulAction = .delete
                DI.selected.selectedFiles = nil
            }
        }
    }

    func renameFileSync(id: UUID, name: String) -> String? {
        let operation = self.core.renameFile(id: id, name: name)

        switch operation {
        case .success(_):
            idsAndFiles[id]?.name = name
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
            let allFiles = DI.core.listFiles()

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
    
    private func postRefreshFiles(_ newFiles: [File]) {
        idsAndFiles = Dictionary(uniqueKeysWithValues: newFiles.map { ($0.id, $0) })
        refreshSuggestedDocs()
        newFiles.forEach {
            if root == nil && $0.id == $0.parent {
                root = $0
            }
        }
        openFileChecks()
        hasRootLoaded = true
    }

    private func openFileChecks() {
        if let selectedFolder = DI.workspace.selectedFolder {
            let maybeMeta = idsAndFiles[selectedFolder]
            
            if maybeMeta == nil {
                DI.workspace.selectedFolder = nil
            }
        }
    }

    public func createDoc(maybeParent: UUID? = nil, isDrawing: Bool) {
        DispatchQueue.global(qos: .userInitiated).async {
            let parent = maybeParent ?? {
                #if os(iOS)
                self.parent?.id ?? self.root!.id
                #else
                DI.workspace.selectedFolder ?? self.root!.id
                #endif
            }()
            
            let fileExt = isDrawing ? ".svg" : ".md"
            var attempt = 0
            
            while(true) {
                let name: String = attempt != 0 ? "untitled-\(attempt)\(fileExt)" : "untitled\(fileExt)"
                
                switch self.core.createFile(name: name, dirId: parent, isFolder: false) {
                case .success(let meta):
                    self.refresh()
                    DispatchQueue.main.sync {
                        DI.workspace.requestOpenDoc(meta.id)
                        #if os(iOS)
                        DI.files.intoChildDirectory(meta)
                        #endif
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
            DI.workspace.selectedFolder ?? root!.id
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
            DI.workspace.selectedFolder ?? root!.id
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
    
    public func getFileByPath(path: String) -> File? {
        switch core.getFileByPath(path: path) {
        case .success(let file):
            return file
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
    
    public func getFolderPaths() -> [String]? {
        switch DI.core.listFolderPaths() {
        case .success(let paths):
            return paths.map({ String($0.dropFirst()) }).sorted()
        case .failure(_):
            return nil
        }
    }
    
    public static func metaToSystemImage(meta: File) -> String {
        switch meta.fileType {
        case .Document:
            return docExtToSystemImage(name: meta.name)
        case .Folder:
            if meta.shares.count != 0 {
                return "folder.fill.badge.person.crop"
            } else {
                return "folder.fill"
            }
        }
    }
    
    public static func docExtToSystemImage(name: String) -> String {
        guard let ext = name.split(separator: ".").last else {
            return "doc"
        }
        
        return extToSystemImg[String(ext)] ?? "doc"
    }
    
    static let extToSystemImg: [String: String] = [
        "md": "doc.richtext",
        "svg": "doc.text.image",
        "pdf": "doc.on.doc",
        
        "txt": "doc.plaintext",
        "rtf": "doc.plaintext",
        "doc": "doc.plaintext",
        "docx": "doc.plaintext",
        
        "html": "chevron.left.slash.chevron.right",
        "xml": "chevron.left.slash.chevron.right",
        "json": "curlybraces",
        "latex": "sum",
        
        "png": "photo",
        "jpg": "photo",
        "jpeg": "photo",
        "tiff": "photo",
        "heif": "photo",
        "heic": "photo",
        
        "zip": "doc.zipper",
        "tar": "doc.zipper",
        "gz": "doc.zipper",
        "7z": "doc.zipper",
        "bz2": "doc.zipper",
        "xz": "doc.zipper",
        "iso": "doc.zipper",
        
        "log": "scroll",
        "csv": "tablecells"
    ]
}

public enum FileAction {
    case move
    case delete
    case createFolder
    case importFiles
    case acceptedShare
}
