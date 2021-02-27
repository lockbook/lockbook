import Foundation
import SwiftLockbookCore
import SwiftUI
import PencilKit

struct DrawingLoader: View {
    
    @ObservedObject var model: DrawingModel
    
    let tool: PKToolPicker = PKToolPicker()
    
    var body: some View {
        switch model.originalDrawing {
        case .some(let drawing):
            DrawingView(drawing: drawing, toolPicker: tool, onChange: model.drawingModelChanged)
        case .none:
            ProgressView().onAppear {
                model.loadDrawing()
            }
        }
    }
    
}

class DrawingModel: ObservableObject {
    @ObservedObject var core: GlobalState
    @Published var originalDrawing: PKDrawing? = .none
    var errors: String? = .none
    let meta: FileMetadata
    var shouldLoadDrawing: Bool = true
    
    init(core: GlobalState, meta: FileMetadata) {
        self.core = core
        self.meta = meta
    }
    
    func drawingModelChanged(updatedDrawing: PKDrawing) {
        shouldLoadDrawing = false
        self.originalDrawing = .none
        DispatchQueue.main.asyncAfter(deadline: .now() + 1, execute: {
            self.originalDrawing = Drawing(from: updatedDrawing).getPKDrawing()
        })
    }
    
    func loadDrawing() {
        if shouldLoadDrawing {
            DispatchQueue.global(qos: .userInitiated).async {
                switch self.core.api.readDrawing(id: self.meta.id) {
                case .success(let drawing):
                    DispatchQueue.main.async {
                        self.originalDrawing = drawing.getPKDrawing()
                    }
                case .failure(let drawingError):
                    self.errors = drawingError.message
                }
            }
        }
    }
}
