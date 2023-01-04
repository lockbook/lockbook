import SwiftUI
import MetalKit

public struct MetalView: NSViewRepresentable {
    let mtkView = CustomMTK()
    let frameManager: FrameManager

    public init() {
        self.frameManager = FrameManager(mtkView)
    }
    
    public func makeNSView(context: NSViewRepresentableContext<MetalView>) -> MTKView {
        mtkView.delegate = self.frameManager
        return mtkView
    }
    
    public func updateNSView(_ nsView: MTKView, context: NSViewRepresentableContext<MetalView>) {
    }
}
