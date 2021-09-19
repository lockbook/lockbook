import SwiftUI
import SwiftLockbookCore
import PencilKit

struct DocumentView: View {
        
    let meta: ClientFileMetadata
    
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
        } else {
            if let type = model.type {
                switch type {
                case .Image:
                    if let img = model.image {
                        ScrollView {
                            img
                        }
                    }
                #if os(iOS)
                case .Drawing:
                    DrawingView(
                        model: model,
                        toolPicker: toolbar
                    )
                    .navigationTitle(Text(meta.name))
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
                    } // TODO there needs to be a 20 horiz padding here on iOS
                case .Unknown:
                    Text("\(meta.name) cannot be opened on this device.")
                }
            }
        }
    }
}
