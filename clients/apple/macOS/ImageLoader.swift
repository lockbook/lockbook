import SwiftUI
import SwiftLockbookCore

struct ImageLoader: View {
    @ObservedObject var model: ImageModel
    let meta: FileMetadata
    @State var image: NSImage?

    var body: some View {
        if let img = model.image, model.meta?.id == meta.id {
            Image(nsImage: img)
        } else {
            ProgressView()
                .onAppear {
                    model.loadDrawing(meta: meta)
                }
        }
    }
}


class ImageModel: ObservableObject {
    @Published var image: NSImage? = .none
    @Published var meta: FileMetadata? = .none
    let read: (UUID) -> FfiResult<Data, ExportDrawingError>

    init(read: @escaping (UUID) -> FfiResult<Data, ExportDrawingError>) {
        self.read = read
    }

    func loadDrawing(meta: FileMetadata) {
        self.meta = meta
        self.image = .none
        DispatchQueue.main.async {
            switch self.read(meta.id) {
            case .success(let data):
                if let nsImage = NSImage(data: Data(data)) {
                    self.image = nsImage
                } else {
                    print("Could not make NSImage from Data!")
                }
            case .failure(let err):
                print(err)
            }
        }
    }

    func closeImage(meta: FileMetadata) {
        self.meta = .none
        self.image = .none
    }
}
