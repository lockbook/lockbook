import SwiftUI
import PencilKit
import SwiftLockbookCore
import Combine

struct DrawingView: UIViewRepresentable {
    
    @State var drawing: PKDrawing = PKDrawing()
    @State var zoom: CGFloat = 1
    @ObservedObject var toolPicker: ToolbarModel

    let onChange: (PKDrawing) -> Void
    
    func makeUIView(context: Context) -> PKCanvasView {
        let view = PKCanvasView()
        view.drawing = drawing
        view.tool = toolPicker.currentTool

        view.isOpaque = false
        view.backgroundColor = .clear
        view.delegate = context.coordinator
        
        view.minimumZoomScale = 1.0
        view.maximumZoomScale = 10.0
        view.contentSize = CGSize(width: 2125, height: 2750)
        view.becomeFirstResponder()

        return view
    }
    
    func updateUIView(_ view: PKCanvasView, context: Context) {
        view.tool = toolPicker.currentTool
        view.isRulerActive = toolPicker.isRulerShowing
    }
    
    class Coordinator: NSObject, PKCanvasViewDelegate {
        @Binding var drawing: PKDrawing
        @Binding var scaleFactor: CGFloat
        let onChange: (PKDrawing) -> ()
        
        init(drawing: Binding<PKDrawing>, scaleFactor: Binding<CGFloat>, onChange: @escaping (PKDrawing) -> Void) {
            _drawing = drawing
            _scaleFactor = scaleFactor
            self.onChange = onChange
        }
        
        func canvasViewDrawingDidChange(_ canvasView: PKCanvasView) {
            self.drawing = canvasView.drawing
            onChange(self.drawing)
        }
        
        func viewForZooming(in scrollView: UIScrollView) -> UIView? {
            return scrollView as! PKCanvasView
        }
        
        func scrollViewDidZoom(_ scrollView: UIScrollView) {
            scaleFactor = scrollView.zoomScale
        }
    }
    
    func makeCoordinator() -> Coordinator {
        return Coordinator(drawing: $drawing, scaleFactor: $zoom, onChange: onChange)
    }
    
}

struct Drawing_Previews: PreviewProvider {
    static var previews: some View {
        HStack {}
        // Drawing(core: GlobalState(), meta: FakeApi().fileMetas[0])
    }
}
