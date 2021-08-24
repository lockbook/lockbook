import SwiftUI
import SwiftLockbookCore
import Combine

struct ImageLoader: View {
    @EnvironmentObject var model:  ImageModel
    let meta: ClientFileMetadata
    @State var deleted: ClientFileMetadata?

    var body: some View {
        Group {
            if (deleted != meta) {
                if let img = model.image, model.meta?.id == meta.id {
                    img
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
    }
}
