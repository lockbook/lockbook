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
                .title(meta.decryptedName)
        } else if model.error != "" {
            Text("errors while loading: \(model.error)")
        } else if model.deleted {
            Text("\(meta.decryptedName) was deleted.")
        } else {
            if let type = model.type {
                switch type {
                case .Image:
                    if let img = model.image {
                        ScrollView([.horizontal, .vertical]) {
                            img
                        }.title(meta.decryptedName)
                    }
                #if os(iOS)
                case .Drawing:
                    DrawingView(
                        model: model,
                        toolPicker: toolbar
                    )
                    .navigationBarTitle(meta.decryptedName, displayMode: .inline)
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
                        EditorView(
                            frame: geo.frame(in: .local)
                        )
                    }
                    .title(meta.decryptedName)
                        
                case .Unknown:
                    Text("\(meta.decryptedName) cannot be opened on this device.")
                        .title(meta.decryptedName)
                }
            }
        }
    }
}

extension View {
    func title(_ name: String) -> some View {
        #if os(macOS)
            return self.navigationTitle(name)
        #else
        return self.navigationBarTitle(name, displayMode: .inline)
        #endif
    }
}
