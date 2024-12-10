import Foundation
import SwiftUI
import SwiftWorkspace
import Combine

class FileService: ObservableObject {
    let core: Lb

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

    init(_ core: Lb) {
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
                    DI.errors.showError(error)
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
                DI.errors.showError(error)
                
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
                    if error.code != .fileNonexistent {
                        DI.errors.showError(error)
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
        let operation = self.core.renameFile(id: id, newName: name)

        switch operation {
        case .success(_):
            idsAndFiles[id]?.name = name
            return nil
        case .failure(let error):
            return error.msg
        }
    }

    func filesToExpand(pathToRoot: [File], currentFile: File) -> [File] {
        if(currentFile.isRoot) {
            return []
        }

        let parentFile = idsAndFiles[currentFile.parent]!

        var pathToRoot = filesToExpand(pathToRoot: pathToRoot, currentFile: parentFile)

        if(currentFile.type == .folder) {
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
                    switch self.core.getFile(id: id) {
                    case .success(let meta):
                        suggestedDocs.append(meta)
                    case .failure(let error):
                        if error.code != .fileNonexistent {
                            DI.errors.showError(error)
                        }
                    }
                }
                    
                DispatchQueue.main.async {
                    self.suggestedDocs = suggestedDocs
                }
            case .failure(let error):
                DI.errors.showError(error)
            }
        }
    }

    func refresh() {
        DispatchQueue.global(qos: .userInteractive).async {
            let allFiles = DI.core.listMetadatas()

            DispatchQueue.main.async {
                switch allFiles {
                case .success(let files):
                    self.postRefreshFiles(files)
                case .failure(let error):
                    DI.errors.showError(error)
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

                switch self.core.createFile(name: name, parent: parent, fileType: .document) {
                case .success(let meta):
                    self.refresh()
                    DispatchQueue.main.sync {
                        DI.workspace.requestOpenDoc(meta.id)
                        #if os(iOS)
                        DI.files.intoChildDirectory(meta)
                        #endif
                    }
                    
                    return
                case .failure(let error):
                    if error.code == .pathTaken {
                        attempt += 1
                    } else {
                        DI.errors.showError(error)
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
        
        switch core.createFile(name: name, parent: realParent, fileType: .folder) {
        case .success(_):
            refresh()
            return nil
        case .failure(let error):
            return error.msg
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
            DI.errors.showError(err)
            return nil
        }
    }
    
    public func getFileByPath(path: String) -> File? {
        switch core.getByPath(path: path) {
        case .success(let file):
            return file
        case .failure(let err):
            DI.errors.showError(err)
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
        switch meta.type {
        case .document:
            return docExtToSystemImage(name: meta.name)
        case .folder:
            if meta.shares.count != 0 {
                return "folder.fill.badge.person.crop"
            } else {
                return "folder.fill"
            }
        case .link(_):
            return "folder.fill"
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
