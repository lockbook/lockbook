import Foundation
import SwiftLockbookCore

class FileService: ObservableObject {
    let core: LockbookApi
    
    @Published var root: ClientFileMetadata? = nil
    @Published var files: [ClientFileMetadata] = []
    
    init(_ core: LockbookApi) {
        self.core = core

        refresh()
    }
    
    // TODO in the future we should pop one of these bad boys up during this operation
    // https://github.com/elai950/AlertToast
    func moveFile(id: UUID, newParent: UUID) {
        DispatchQueue.global(qos: .userInteractive).async {
            let operation = self.core.moveFile(id: id, newParent: newParent)
            
            DispatchQueue.main.async {
                switch operation {
                case .success(_) :
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
    
    func renameFile(id: UUID, name: String) {
        DispatchQueue.global(qos: .userInteractive).async {
            let operation = self.core.renameFile(id: id, name: name)

            DispatchQueue.main.async {
                switch operation {
                case .success(_) :
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
    
    func refresh() {
        DispatchQueue.global(qos: .userInteractive).async {
            let allFiles = self.core.listFiles()
            let root = self.core.getRoot()
            
            DispatchQueue.main.async {
                switch root {
                case .success(let root):
                    self.root = root
                case .failure(let error):
                    DI.errors.handleError(error)
                }
                
                switch allFiles {
                case .success(let files):
                    self.files = files
                    self.files.forEach { self.notifyDocumentChanged($0) }
                case .failure(let error):
                    DI.errors.handleError(error)
                }
            }
        }
    }
    
    private func notifyDocumentChanged(_ meta: ClientFileMetadata) {
        if let openDrawingMeta = DI.openDrawing.meta, meta.id == openDrawingMeta.id, meta.contentVersion != openDrawingMeta.contentVersion {
            DI.openDrawing.reloadDocumentIfNeeded(meta: openDrawingMeta)
        }
        if let openDocumentMeta = DI.openDocument.meta, meta.id == openDocumentMeta.id, meta.contentVersion != openDocumentMeta.contentVersion {
            DispatchQueue.main.async {
                DI.openDocument.reloadDocumentIfNeeded(meta: openDocumentMeta)
            }
        }
    }
}
