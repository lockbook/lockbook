import Foundation
import SwiftLockbookCore
import SwiftUI
import PencilKit
import Combine

struct DrawingLoader: View {

    @ObservedObject var model: DrawingModel
    @ObservedObject var toolbar: ToolbarModel
    let meta: ClientFileMetadata
    let deleteChannel: PassthroughSubject<ClientFileMetadata, Never>
    @State var deleted: ClientFileMetadata?

    var body: some View {
        Group {
            if (deleted != meta) {
                switch model.originalDrawing {
                case .some(let drawing):
                    GeometryReader { geom in
                        DrawingView(frame: geom.frame(in: .local), drawing: drawing, toolPicker: toolbar, onChange: { (ud: PKDrawing) in model.drawingModelChanged(meta: meta, updatedDrawing: ud) })
                            .navigationTitle(meta.name)
                            .toolbar {
                                ToolbarItemGroup(placement: .bottomBar) {
                                    Spacer()
                                    DrawingToolbar(toolPicker: toolbar)
                                    Spacer()
                                }
                            }
                            .onDisappear {
                                model.closeDrawing()
                            }
                    }
                case .none:
                    ProgressView()
                        .onAppear {
                            model.loadDrawing(meta: meta)
                        }
                }
            } else {
                Text("\(meta.name) file has been deleted")
            }
        }
        .onReceive(deleteChannel) { deletedMeta in
            if (deletedMeta.id == meta.id) {
                deleted = deletedMeta
            }
        }
    }
}

class DrawingModel: ObservableObject {
    @Published var originalDrawing: PKDrawing? = .none
    @Published var meta: ClientFileMetadata? = .none
    var errors: String? = .none
    let write: (UUID, Drawing) -> FfiResult<SwiftLockbookCore.Empty, WriteToDocumentError>
    let read: (UUID) -> FfiResult<Drawing, ReadDocumentError>
    var writeListener: () -> Void = {}

    init(write: @escaping (UUID, Drawing) -> FfiResult<SwiftLockbookCore.Empty, WriteToDocumentError>, read: @escaping (UUID) -> FfiResult<Drawing, ReadDocumentError>) {
        self.write = write
        self.read = read
    }

    func drawingModelChanged(meta: ClientFileMetadata, updatedDrawing: PKDrawing) {
        originalDrawing = updatedDrawing
        DispatchQueue.global(qos: .userInitiated).async {
            print(self.write(meta.id, Drawing(from: updatedDrawing))) // TODO handle
            self.writeListener()
        }
    }

    func loadDrawing(meta: ClientFileMetadata) {
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
