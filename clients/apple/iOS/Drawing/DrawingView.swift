import SwiftUI
import PencilKit
import SwiftLockbookCore
import Combine

struct DrawingView: UIViewRepresentable {
    @Environment(\.colorScheme) var colorScheme
    let frame: CGRect
    @State var drawing: PKDrawing = PKDrawing()
    @State var zoom: CGFloat = 1
    @ObservedObject var toolPicker: ToolbarModel
    let pencilInteraction = UIPencilInteraction()
    let view = PKCanvasView()
    let gridView = GridUIView()
    let backgroundView = UIView()

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

        backgroundView.backgroundColor = colorScheme == .light ? .white : .black
        backgroundView.frame = CGRect(origin: CGPoint(x: 0, y: 0), size: self.frame.size)
        view.addSubview(backgroundView)
        view.sendSubviewToBack(backgroundView)

        gridView.backgroundColor = .clear
        gridView.frame = self.frame
        view.addSubview(gridView)
        view.sendSubviewToBack(gridView)


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
        let didZoom: () -> ()

        init(drawing: Binding<PKDrawing>, scaleFactor: Binding<CGFloat>, onChange: @escaping (PKDrawing) -> Void, didZoom: @escaping () -> Void) {
            _drawing = drawing
            _scaleFactor = scaleFactor
            self.onChange = onChange
            self.didZoom = didZoom
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
            didZoom()
        }
    }

    func makeCoordinator() -> Coordinator {
        Coordinator(drawing: $drawing, scaleFactor: $zoom, onChange: onChange, didZoom: {
            backgroundView.frame = CGRect(origin: CGPoint(x: 0, y: 0), size: view.contentSize)
            let gsz = view.visibleSize.multiply(factor: 2)
            gridView.frame = CGRect(origin: CGPoint(x: -gsz.width/3, y: -gsz.height/3), size: gsz)
        })
    }

}

struct Drawing_Previews: PreviewProvider {
    static let core = GlobalState()
    static let toolbar = ToolbarModel()
    static let dm = DrawingModel(write: { _, _ in .failure(.init(unexpected: "LAZY"))}, read: { _ in .failure(.init(unexpected: "LAZY"))})
    static let dc = PassthroughSubject<FileMetadata, Never>()

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
