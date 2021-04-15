import SwiftUI
import SwiftLockbookCore
import Combine

struct ImageLoader: View {
    @ObservedObject var model: ImageModel
    let meta: FileMetadata
    @State var image: NSImage?
    let deleteChannel: PassthroughSubject<FileMetadata, Never>
    @State var deleted: FileMetadata?

    var body: some View {
        Group {
            if (deleted != meta) {
                if let img = model.image, model.meta?.id == meta.id {
                    Image(nsImage: img)
                } else {
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
