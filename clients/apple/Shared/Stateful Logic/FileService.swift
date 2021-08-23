import Foundation
import SwiftLockbookCore

class FileService: ObservableObject {
    let core: LockbookApi
    let errors: UnexpectedErrorService
    let openDrawing: DrawingModel
    let openImage: ImageModel
    let openDocument: Content
    
    @Published var root: ClientFileMetadata? = nil
    @Published var files: [ClientFileMetadata] = []
    
    init(_ core: LockbookApi, _ openDrawing: DrawingModel, _ openImage: ImageModel, _ openDocument: Content, _ errors: UnexpectedErrorService) {
        self.core = core
        self.openDrawing = openDrawing
        self.openImage = openImage
        self.openDocument = openDocument
        self.errors = errors
        
        refresh()
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
                    self.errors.handleError(error)
                }
                
                switch allFiles {
                case .success(let files):
                    self.files = files
                    self.files.forEach { self.notifyDocumentChanged($0) }
                case .failure(let error):
                    self.errors.handleError(error)
                }
            }
        }
    }
    
    private func notifyDocumentChanged(_ meta: ClientFileMetadata) {
        if let openDrawingMeta = openDrawing.meta, meta.id == openDrawingMeta.id, meta.contentVersion != openDrawingMeta.contentVersion {
            self.openDrawing.reloadDocumentIfNeeded(meta: openDrawingMeta)
        }
        if let openDocumentMeta = self.openDocument.meta, meta.id == openDocumentMeta.id, meta.contentVersion != openDocumentMeta.contentVersion {
            DispatchQueue.main.async {
                self.openDocument.reloadDocumentIfNeeded(meta: openDocumentMeta)
            }
        }
    }
}
