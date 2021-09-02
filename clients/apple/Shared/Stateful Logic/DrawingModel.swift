import Foundation
import SwiftLockbookCore
import PencilKit

class DrawingModel: ObservableObject {
    let core: LockbookApi
    
    @Published var loadDrawing: PKDrawing? = .none
    @Published var meta: ClientFileMetadata? = .none
    @Published var deleted: Bool = false
    var errors: String? = .none
    
    init(_ core: LockbookApi) {
        self.core = core
    }
    
    func drawingModelChanged(meta: ClientFileMetadata, updatedDrawing: PKDrawing) {
//        saveDrawing = updatedDrawing
        DispatchQueue.global(qos: .userInitiated).async {
            switch self.core.writeDrawing(id: meta.id, content: Drawing(from: updatedDrawing)) {
            case .success(_):
                print("drawing saved successfully")
            case .failure(let error):
                DI.errors.handleError(error)
            }

            DI.sync.documentChangeHappened()
        }
    }
    
    func loadDrawing(meta: ClientFileMetadata) {
        DispatchQueue.main.async {
            switch self.core.readDrawing(id: meta.id) {
            case .success(let drawing):
                self.meta = meta
                self.loadDrawing = PKDrawing(from: drawing)
                self.deleted = false
            case .failure(let drawingError):
                print(drawingError)
                self.errors = drawingError.message
            }
        }
    }
    
    func closeDrawing() {
        self.meta = .none
        self.loadDrawing = .none
    }
    
    func reloadDocumentIfNeeded(meta: ClientFileMetadata) {
        switch self.loadDrawing {
        case .some(_):
            switch self.core.readDrawing(id: meta.id) {
            case .success(let coreDrawing):
                self.closeDrawing()
                self.meta = meta
                self.loadDrawing = PKDrawing(from: coreDrawing)
            case .failure(let err):
                print(err)
            }
        case .none:
            print("No open drawing")
        }
        
    }
}
