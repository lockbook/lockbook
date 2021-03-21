import Foundation
import SwiftLockbookCore
import SwiftUI
import PencilKit

struct DrawingLoader: View {

    @ObservedObject var model: DrawingModel
    @ObservedObject var toolbar: ToolbarModel
    let meta: FileMetadata

    var body: some View {
        switch model.originalDrawing {
        case .some(let drawing):
            DrawingView(drawing: drawing, toolPicker: toolbar, onChange: { (ud: PKDrawing) in model.drawingModelChanged(meta: meta, updatedDrawing: ud) })
                .navigationTitle(String("\(meta.name) \(meta.contentVersion)"))
                .toolbar {
                    ToolbarItemGroup(placement: .bottomBar) {
                        Spacer()
                        DrawingToolbar(toolPicker: toolbar)
                        Spacer()
                    }
                }
                .onDisappear {
                    model.closeDrawing(meta: meta)
                }
        case .none:
            ProgressView()
                .onAppear {
                    model.loadDrawing(meta: meta)
                }
        }
    }
}

class DrawingModel: ObservableObject {
    @Published var originalDrawing: PKDrawing? = .none
    @Published var meta: FileMetadata? = .none
    var errors: String? = .none
    let write: (UUID, Drawing) -> FfiResult<Empty, WriteToDocumentError>
    let read: (UUID) -> FfiResult<Drawing, ReadDocumentError>
    var writeListener: () -> Void = {}

    init(write: @escaping (UUID, Drawing) -> FfiResult<Empty, WriteToDocumentError>, read: @escaping (UUID) -> FfiResult<Drawing, ReadDocumentError>) {
        self.write = write
        self.read = read
    }

    func drawingModelChanged(meta: FileMetadata, updatedDrawing: PKDrawing) {
        originalDrawing = updatedDrawing
        DispatchQueue.global(qos: .userInitiated).async {
            print(self.write(meta.id, Drawing(from: updatedDrawing))) // TODO handle
            self.writeListener()
        }
    }

    func loadDrawing(meta: FileMetadata) {
        DispatchQueue.main.async {
            switch self.read(meta.id) {
            case .success(let drawing):
                self.meta = meta
                self.originalDrawing = PKDrawing(from: drawing)
            case .failure(let drawingError):
                print(drawingError)
                self.errors = drawingError.message
            }
        }
    }

    func closeDrawing(meta: FileMetadata) {
        self.meta = .none
        self.originalDrawing = .none
    }
}
