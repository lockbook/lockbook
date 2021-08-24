import Foundation
import SwiftLockbookCore
import SwiftUI
import PencilKit
import Combine

struct DrawingLoader: View {
    
    @EnvironmentObject var model: DrawingModel
    @EnvironmentObject var toolbar: ToolbarModel
    
    let meta: ClientFileMetadata
    @State var deleted: ClientFileMetadata?
    
    var body: some View {
        Group {
            if (deleted != meta) {
                switch model.originalDrawing {
                case .some(let drawing):
                    DrawingView(drawing: drawing, toolPicker: toolbar, onChange: { (ud: PKDrawing) in model.drawingModelChanged(meta: meta, updatedDrawing: ud) })
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
    }
}
