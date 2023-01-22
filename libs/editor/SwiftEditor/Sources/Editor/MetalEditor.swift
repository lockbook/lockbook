import SwiftUI
import MetalKit
import Combine

public struct MetalView: NSViewRepresentable {
    @Binding public var reloadText: Bool
    public let mtkView: CustomMTK
    public let frameManager: FrameManager
    
    public init(mtkView: CustomMTK, reloadText: Binding<Bool>, frameManager: FrameManager) {
        self._reloadText = reloadText
        self.frameManager = frameManager
        self.mtkView = mtkView
    }
    
    public func makeNSView(context: NSViewRepresentableContext<MetalView>) -> MTKView {
        mtkView.delegate = self.frameManager
        return mtkView
    }
    
    public func updateNSView(_ nsView: MTKView, context: NSViewRepresentableContext<MetalView>) {
        if reloadText {
            frameManager.reloadText()
        }
    }
}
