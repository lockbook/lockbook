import Foundation
import SwiftLockbookCore
import PencilKit

class DrawingModel: ObservableObject {
    @Published var originalDrawing: PKDrawing? = .none
    @Published var meta: ClientFileMetadata? = .none
    @Published var deleted: Bool = false
    var errors: String? = .none
    // TODO take this in via DI instead
    let write: (UUID, Drawing) -> FfiResult<SwiftLockbookCore.Empty, WriteToDocumentError>
    let read: (UUID) -> FfiResult<Drawing, ReadDocumentError>
    
    init(write: @escaping (UUID, Drawing) -> FfiResult<SwiftLockbookCore.Empty, WriteToDocumentError>, read: @escaping (UUID) -> FfiResult<Drawing, ReadDocumentError>) {
        self.write = write
        self.read = read
    }
    
    func drawingModelChanged(meta: ClientFileMetadata, updatedDrawing: PKDrawing) {
        originalDrawing = updatedDrawing
        DispatchQueue.global(qos: .userInitiated).async {
            print(self.write(meta.id, Drawing(from: updatedDrawing))) // TODO handle
            DI.sync.documentChangeHappened()
        }
    }
    
    func loadDrawing(meta: ClientFileMetadata) {
        DispatchQueue.main.async {
            switch self.read(meta.id) {
            case .success(let drawing):
                self.meta = meta
                self.originalDrawing = PKDrawing(from: drawing)
                self.deleted = false
            case .failure(let drawingError):
                print(drawingError)
                self.errors = drawingError.message
            }
        }
    }
    
    func closeDrawing() {
        self.meta = .none
        self.originalDrawing = .none
    }
    
    func reloadDocumentIfNeeded(meta: ClientFileMetadata) {
        switch self.originalDrawing {
        case .some(let currentDrawing):
            switch self.read(meta.id) {
            case .success(let coreDrawing):
                if Drawing(from: currentDrawing) != coreDrawing { /// Close the document
                    print("reload")
                    self.closeDrawing()
                    self.meta = meta
                    self.originalDrawing = PKDrawing(from: coreDrawing)
                }
            case .failure(let err):
                print(err)
            }
        case .none:
            print("No open drawing")
        }
        
    }
}
