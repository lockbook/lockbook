import SwiftUI
import PencilKit
import SwiftLockbookCore
import Combine

struct DrawingView: UIViewRepresentable {
    @Environment(\.colorScheme) var colorScheme
    @State var drawing: PKDrawing = PKDrawing()
    @State var zoom: CGFloat = 1
    @ObservedObject var toolPicker: ToolbarModel
    let pencilInteraction = UIPencilInteraction()
    let view = PKCanvasView()

    let onChange: (PKDrawing) -> Void

    func makeUIView(context: Context) -> PKCanvasView {
        view.drawing = drawing
        view.tool = toolPicker.currentTool

        view.isOpaque = false
        view.backgroundColor = .clear
        view.delegate = context.coordinator

        view.minimumZoomScale = 0.1
        view.maximumZoomScale = 20.0
        view.contentSize = CGSize(width: 2125, height: 2750)
        view.becomeFirstResponder()

        pencilInteraction.delegate = toolPicker
        view.addInteraction(pencilInteraction)

        let imageView = UIImageView(image: UIImage(named: "grid")?.resizableImage(withCapInsets: UIEdgeInsets.init(top: 0, left: 0, bottom: 0, right: 0), resizingMode: .tile))
        imageView.alpha = 0.3
        imageView.frame = CGRect(x: 0, y: 0, width: view.contentSize.width, height: view.contentSize.height)
        let contentView = view.subviews[0]
        contentView.addSubview(imageView)
        contentView.sendSubviewToBack(imageView)
        
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
            drawing = canvasView.drawing
            onChange(drawing)
        }

        func viewForZooming(in scrollView: UIScrollView) -> UIView? {
            scrollView as! PKCanvasView
        }

        func scrollViewDidZoom(_ scrollView: UIScrollView) {
            scaleFactor = scrollView.zoomScale
            let offsetX = max((scrollView.bounds.width - scrollView.contentSize.width) * 0.5, 0)
            let offsetY = max((scrollView.bounds.height - scrollView.contentSize.height) * 0.5, 0)
            scrollView.contentInset = UIEdgeInsets(top: offsetY, left: offsetX, bottom: 0, right: 0)
        }
    }

    func makeCoordinator() -> Coordinator {
        Coordinator(drawing: $drawing, scaleFactor: $zoom, onChange: onChange)
    }

}

struct Drawing_Previews: PreviewProvider {
    static let core = GlobalState()
    static let toolbar = ToolbarModel()
    static let dm = DrawingModel(write: { _, _ in .failure(.init(unexpected: "LAZY"))}, read: { _ in .failure(.init(unexpected: "LAZY"))})
    static let dc = PassthroughSubject<ClientFileMetadata, Never>()

    static var previews: some View {
        DrawingLoader(model: dm, toolbar: toolbar, meta: core.files[0], deleteChannel: dc)
            .onAppear {
                dm.originalDrawing = PKDrawing()
                toolbar.selectedColor = .Red
            }
    }
}

extension CGSize {
    func multiply(factor: CGFloat) -> CGSize {
        .init(width: self.width*factor, height: self.height*factor)
    }
}
