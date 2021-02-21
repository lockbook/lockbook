import Foundation
import SwiftLockbookCore
import SwiftUI
import PencilKit

struct DrawingLoader: View {
    
    @ObservedObject var model: DrawingModel
    
    let tool: PKToolPicker = PKToolPicker()
    
    var body: some View {
        switch model.pkDrawing {
        case .some(let drawing):
            DrawingView(drawing: drawing, toolPicker: tool)
        case .none:
            ProgressView().onAppear {
                model.loadDrawing()
            }
        }
    }
    
}

class DrawingModel: ObservableObject {
    @ObservedObject var core: GlobalState
    @Published var pkDrawing: PKDrawing? = .none
    var errors: String? = .none
    let meta: FileMetadata
    
    init(core: GlobalState, meta: FileMetadata) {
        self.core = core
        self.meta = meta
    }

    func loadDrawing() {
        DispatchQueue.global(qos: .userInitiated).async {
            switch self.core.api.readDrawing(id: self.meta.id) {
            case .success(let drawing):
                DispatchQueue.main.async {
                    self.pkDrawing = drawing.getPKDrawing()
                }
            case .failure(let drawingError):
                self.errors = drawingError.message
            }
        }
    }
}
