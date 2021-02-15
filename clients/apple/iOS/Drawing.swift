import SwiftUI
import PencilKit
import SwiftLockbookCore
import Combine

struct Drawing: UIViewRepresentable {
    
    @ObservedObject var core: GlobalState
    let meta: FileMetadata
    @State var drawing: PKDrawing = PKDrawing()
    @State var zoom: CGFloat = 1
    
    init(core: GlobalState, meta: FileMetadata) {
        self.core = core
        self.meta = meta
        
        let lbDrawing = core.api.readDrawing(id: meta.id)
        print(lbDrawing)
        self.drawing = PKDrawing()
    }
    
    // How you'll ultimately replace the PKToolPicker
    // https://sarunw.com/posts/move-view-around-with-drag-gesture-in-swiftui/
    let toolPicker: PKToolPicker = PKToolPicker()
    
    func makeUIView(context: Context) -> PKCanvasView {
        let view = PKCanvasView()
        view.drawing = drawing
        view.tool = toolPicker.selectedTool
        
        view.isOpaque = false
        view.backgroundColor = .clear
        view.delegate = context.coordinator
        
        view.minimumZoomScale = 1.0
        view.maximumZoomScale = 10.0
        
        toolPicker.setVisible(true, forFirstResponder: view)
        toolPicker.addObserver(view)
        view.becomeFirstResponder()
        
        return view
    }
    
    func updateUIView(_ view: PKCanvasView, context: Context) {
        view.tool = toolPicker.selectedTool
    }
    
    static func dismantleUIView(_ uiView: PKCanvasView, coordinator: Coordinator) {
        coordinator.toolPicker.setVisible(false, forFirstResponder: uiView)
    }
    
    class Coordinator: NSObject, PKCanvasViewDelegate {
        @Binding var drawing: PKDrawing
        @Binding var scaleFactor: CGFloat
        var toolPicker: PKToolPicker
        
        init(drawing: Binding<PKDrawing>, scaleFactor: Binding<CGFloat>, toolPicker: PKToolPicker) {
            _drawing = drawing
            _scaleFactor = scaleFactor
            self.toolPicker = toolPicker
        }
        
        func canvasViewDrawingDidChange(_ canvasView: PKCanvasView) {
            self.drawing = canvasView.drawing
        }
        
        func viewForZooming(in scrollView: UIScrollView) -> UIView? {
            return scrollView as! PKCanvasView
        }
        
        func scrollViewDidZoom(_ scrollView: UIScrollView) {
            scaleFactor = scrollView.zoomScale
        }
    }
    
    func makeCoordinator() -> Coordinator {
        return Coordinator(drawing: $drawing, scaleFactor: $zoom, toolPicker: toolPicker)
    }
    
}

struct Drawing_Previews: PreviewProvider {
    static var previews: some View {
        HStack {}
        // Drawing(core: GlobalState(), meta: FakeApi().fileMetas[0])
    }
}
