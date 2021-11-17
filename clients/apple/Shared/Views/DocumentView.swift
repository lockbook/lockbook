import SwiftUI
import SwiftLockbookCore
import PencilKit

struct DocumentView: View {
        
    let meta: DecryptedFileMetadata
    
    @EnvironmentObject var model: DocumentLoader
    #if os(iOS)
    @EnvironmentObject var toolbar: ToolbarModel
    #endif
    
    var body: some View {
        if meta != model.meta || model.loading {
            ProgressView()
                .onAppear {
                    model.startLoading(meta)
                }
                .navigationTitle(meta.decryptedName)
        } else if model.deleted {
            Text("\(meta.decryptedName) was deleted.")
        } else {
            if let type = model.type {
                switch type {
                case .Image:
                    if let img = model.image {
                        ScrollView([.horizontal, .vertical]) {
                            img
                        }.navigationTitle(meta.decryptedName)
                    }
                #if os(iOS)
                case .Drawing:
                    DrawingView(
                        model: model,
                        toolPicker: toolbar
                    )
                    .navigationTitle(meta.decryptedName)
                    .toolbar {
                        ToolbarItemGroup(placement: .bottomBar) {
                            Spacer()
                            DrawingToolbar(toolPicker: toolbar)
                            Spacer()
                        }
                    }
                #endif
                
                case .Markdown:
                    GeometryReader { geo in
                        NotepadView(
                            model: model,
                            frame: geo.frame(in: .local),
                            theme: LockbookTheme
                        )
                    }.navigationTitle(meta.decryptedName)
                    // TODO there needs to be a 20 horiz padding here on iOS
                case .Unknown:
                    Text("\(meta.decryptedName) cannot be opened on this device.")
                }
            }
        }
    }
}
