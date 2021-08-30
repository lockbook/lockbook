import SwiftUI
import SwiftLockbookCore

class ImageModel: ObservableObject {
    @Published var image: Image? = .none
    @Published var meta: ClientFileMetadata? = .none
    @Published var deleted: Bool = false
    let read: (UUID) -> FfiResult<Data, ExportDrawingError>

    init(read: @escaping (UUID) -> FfiResult<Data, ExportDrawingError>) {
        self.read = read
    }

    func loadDrawing(meta: ClientFileMetadata) {
        self.meta = meta
        self.image = .none
        self.deleted = false
        DispatchQueue.main.async {
            switch self.read(meta.id) {
            case .success(let data):
                if let image = self.getImage(from: data) {
                    self.image = image
                } else {
                    print("Could not make NSImage from Data!")
                }
            case .failure(let err):
                print(err)
            }
        }
    }
    
    func getImage(from: Data) -> Image? {
        #if os(macOS)
        if let nsImage = NSImage(data: from) {
            return Image(nsImage: nsImage)
        } else {
            return .none
        }
        #else
        if let uiImage = UIImage(data: from) {
            return Image(uiImage: uiImage)
        } else {
            return .none
        }
        #endif
    }

    func closeImage(meta: ClientFileMetadata) {
        self.meta = .none
        self.image = .none
    }
}
