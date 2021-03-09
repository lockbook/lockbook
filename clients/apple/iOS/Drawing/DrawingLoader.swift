import Foundation
import SwiftLockbookCore
import SwiftUI
import PencilKit

struct DrawingLoader: View {

    @ObservedObject var model: DrawingModel
    @ObservedObject var toolbar = ToolbarModel()

    var body: some View {
        switch model.originalDrawing {
        case .some(let drawing):
            DrawingView(drawing: drawing, toolPicker: toolbar, onChange: model.drawingModelChanged)
                    .navigationTitle(model.meta.name)
                    .toolbar {
                        ToolbarItemGroup(placement: .bottomBar) {
                            Spacer()
                            DrawingToolbar(toolPicker: toolbar)
                            Spacer()
                        }
                    }
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

    init(core: GlobalState, meta: FileMetadata) {
        self.core = core
        self.meta = meta
    }

    func drawingModelChanged(updatedDrawing: PKDrawing) {
        originalDrawing = updatedDrawing
        DispatchQueue.global(qos: .userInitiated).async {
            print(self.core.api.writeDrawing(id: self.meta.id, content: Drawing(from: updatedDrawing)))
        }
    }

    func loadDrawing() {
        DispatchQueue.global(qos: .userInitiated).async {
            switch self.core.api.readDrawing(id: self.meta.id) {
            case .success(let drawing):
                DispatchQueue.main.async {
                    self.originalDrawing = PKDrawing(from: drawing)
                }
            case .failure(let drawingError):
                self.errors = drawingError.message
            }
        }

    }
}
