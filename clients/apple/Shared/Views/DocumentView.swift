import SwiftUI
import SwiftLockbookCore
import PencilKit

struct DocumentView: View {
    
    let meta: File
    
    @EnvironmentObject var model: DocumentLoader
#if os(iOS)
    @EnvironmentObject var toolbar: ToolbarModel
    @EnvironmentObject var current: CurrentDocument
#endif
    
    var body: some View {
        if meta != model.meta || model.loading {
            ProgressView()
                .onAppear {
                    model.startLoading(meta)
                    
                    if(current.selectedDocument != meta) {
                        current.selectedDocument = meta
                    }
                }
                .title(meta.name)
        } else if model.error != "" {
            Text("errors while loading: \(model.error)")
        } else if model.deleted {
            Text("\(meta.name) was deleted.")
        } else {
            if let type = model.type {
                switch type {
                case .Image:
                    if let img = model.image {
                        ScrollView([.horizontal, .vertical]) {
                            img
                        }.title(meta.name)
                    }
#if os(iOS)
                case .Drawing:
                    DrawingView(
                        model: model,
                        toolPicker: toolbar
                    )
                    .navigationBarTitle(meta.name, displayMode: .inline)
                    .toolbar {
                        ToolbarItemGroup(placement: .bottomBar) {
                            Spacer()
                            DrawingToolbar(toolPicker: toolbar)
                            Spacer()
                        }
                    }
#endif
                    
                case .Markdown:
                    #if os(iOS)
                    GeometryReader { geo in
                        EditorView(
                            frame: geo.frame(in: .local)
                        )
                    }
                    .title(meta.name)
                    #else
                    EditorView().title(meta.name)
                    #endif
                    
                case .Unknown:
                    Text("\(meta.name) cannot be opened on this device.")
                        .title(meta.name)
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
