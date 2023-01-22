import Foundation
import SwiftUI
import SwiftEditor

struct EditorView: View {
    
    let mtkView = CustomMTK()
    let frameManager: FrameManager
    @EnvironmentObject var loader: DocumentLoader
    
    @FocusState var focused: Bool
    
    init() {
        self.frameManager = FrameManager(self.mtkView, DI.documentLoader)
    }
    
    var body: some View {
        MetalView(
            mtkView: mtkView,
            reloadText: $loader.reloadContent,
            frameManager: self.frameManager
        )
        .focused($focused)
        .onAppear {
            focused = true
        }
    }
}
