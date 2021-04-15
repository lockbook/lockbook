import SwiftUI
import PencilKit
import SwiftLockbookCore
import Foundation
import Combine

struct DrawingToolbar: View {

    @Environment(\.colorScheme) var colorScheme
    @ObservedObject var toolPicker: ToolbarModel

    var lasso: some View {
        selectableButton(
                imageName: "lasso",
                selected: toolPicker.lassoSelected,
                onSelect: { toolPicker.currentTool = PKLassoTool() },
                onUnSelect: toolPicker.backToDrawing
        )
    }

    var eraser: some View {
        selectableButton(
                imageName: "square.righthalf.fill",
                selected: toolPicker.eraserSelected,
                onSelect: { toolPicker.currentTool = PKEraserTool(.vector) },
                onUnSelect: toolPicker.backToDrawing
        )
    }

    var ruler: some View {
        selectableButton(
                imageName: "ruler",
                selected: toolPicker.isRulerShowing,
                onSelect: { toolPicker.isRulerShowing.toggle() },
                onUnSelect: { toolPicker.isRulerShowing.toggle() }
        )
    }

    var colors: some View {
        Group {
            colorCircle(.White)
            colorCircle(.Black)
            colorCircle(.Red)
            colorCircle(.Green)
            colorCircle(.Blue)
            colorCircle(.Cyan)
            colorCircle(.Magenta)
            colorCircle(.Yellow)
        }
    }

    var undo: some View {
        Image(systemName: "arrowshape.turn.up.left.circle")
                .imageScale(.large)
                .frame(width: 30, height: 30, alignment: .center)
                .foregroundColor(Color.gray)
                .cornerRadius(3.0)
    }

    var redo: some View {
        Image(systemName: "arrowshape.turn.up.right.circle")
                .imageScale(.large)
                .frame(width: 30, height: 30, alignment: .center)
                .foregroundColor(Color.gray)
                .cornerRadius(3.0)
    }
    
    var slider: some View {
        let formattedFloat = String(format: "%.1f", toolPicker.width)

        return HStack {
            Slider(value: $toolPicker.width, in: 0.8...20)
                .frame(width: 100)
            Text("\(formattedFloat)")
        }

    }

    var body: some View {
        HStack {
            lasso
            eraser
            ruler
            colors
            undo
            redo
            slider
        }
    }

    func colorCircle(_ preDarkModeConversion: ColorAlias) -> AnyView {
        var postDarkModeConversion = preDarkModeConversion

        if colorScheme == .dark {
            if preDarkModeConversion == ColorAlias.White {
                postDarkModeConversion = ColorAlias.Black
            }
            if preDarkModeConversion == ColorAlias.Black {
                postDarkModeConversion = ColorAlias.White
            }
        }

        return AnyView(
                Image(systemName: toolPicker.selectedColor == postDarkModeConversion ? "largecircle.fill.circle" : "circle.fill")
                        .imageScale(.large)
                        .foregroundColor(Color(.fromColorAlias(from: preDarkModeConversion)))
                        .frame(width: 30, height: 30, alignment: .center)
                        .onTapGesture {
                            toolPicker.currentTool = PKInkingTool(.pen, color: .fromColorAlias(from: postDarkModeConversion), width: CGFloat(toolPicker.width))
                            toolPicker.selectedColor = postDarkModeConversion
                        }
        )
    }

    func selectableButton(imageName: String, selected: Bool, onSelect: @escaping () -> Void, onUnSelect: @escaping () -> Void) -> AnyView {
        if selected {
            return AnyView(
                    Image(systemName: imageName)
                            .imageScale(.large)
                            .frame(width: 30, height: 30, alignment: .center)
                            .foregroundColor(Color(UIColor.systemBackground))
                            .background(Color.blue)
                            .cornerRadius(3.0)
                            .onTapGesture(perform: onUnSelect)
            )
        } else {
            return AnyView(
                    Image(systemName: imageName)
                            .imageScale(.large)
                            .frame(width: 30, height: 30, alignment: .center)
                            .foregroundColor(Color.blue)
                            .cornerRadius(3.0)
                            .onTapGesture(perform: onSelect)
            )
        }
    }

}

class ToolbarModel: NSObject, ObservableObject, UIPencilInteractionDelegate {
    static let initialColor: ColorAlias = .Red
    static let initialWidth: Float = 1
    
    var selectedColor: ColorAlias = initialColor

    @Published var currentTool: PKTool = PKInkingTool(.pen, color: .fromColorAlias(from: initialColor), width: CGFloat(initialWidth))
    @Published var isRulerShowing: Bool = false
    @Published var width: Float = initialWidth {      // Setting this to true kicks off a sync
        didSet {
            backToDrawing()
        }
    }

    var lassoSelected: Bool {
        type(of: currentTool) == PKLassoTool.self
    }

    var eraserSelected: Bool {
        type(of: currentTool) == PKEraserTool.self
    }

    func backToDrawing() {
        currentTool = PKInkingTool(.pen, color: .fromColorAlias(from: selectedColor), width: CGFloat(width))
    }

    func pencilInteractionDidTap(_ interaction: UIPencilInteraction) {
        if eraserSelected {
            backToDrawing()
        } else {
            currentTool = PKEraserTool(.vector)
        }
    }
}


struct Toolbar_Preview: PreviewProvider {
    static let core = GlobalState()
    static let toolbar = ToolbarModel()
    static let dm = DrawingModel(write: { _, _ in .failure(.init(unexpected: "LAZY"))}, read: { _ in .failure(.init(unexpected: "LAZY"))})
    static let dc = PassthroughSubject<FileMetadata, Never>()

    static var previews: some View {
        NavigationView {
            HStack {
            }
            DrawingLoader(model: dm, toolbar: toolbar, meta: core.files[0], deleteChannel: dc)
                    .onAppear {
                        dm.originalDrawing = PKDrawing()
                        toolbar.selectedColor = .Red
                    }
        }
    }

}
