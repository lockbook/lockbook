import Foundation
import SwiftLockbookCore
import SwiftUI
import PencilKit
import Combine

struct DrawingLoader: View {
    
    @EnvironmentObject var model: DrawingModel
    @EnvironmentObject var toolbar: ToolbarModel
    
    let meta: ClientFileMetadata
    
    var body: some View {
        Group {
            switch model.loadDrawing {
            case .some(let drawing):
                if model.deleted {
                    Text("\(meta.name) file has been deleted")
                } else {
                    DrawingView(drawingToLoad: drawing, toolPicker: toolbar, onChange: { (ud: PKDrawing) in model.drawingModelChanged(meta: meta, updatedDrawing: ud) })
                        .navigationTitle(meta.name)
                        .toolbar {
                            ToolbarItemGroup(placement: .bottomBar) {
                                Spacer()
                                DrawingToolbar(toolPicker: toolbar)
                                Spacer()
                            }
                        }
                }
            case .none:
                ProgressView()
                    .onAppear {
                        model.loadDrawing(meta: meta)
                    }
            }
        }
    }
}
