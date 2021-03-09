import SwiftUI
import PencilKit
import SwiftLockbookCore
import Foundation

struct DrawingToolbar: View {

    @Environment(\.colorScheme) var colorScheme
    @ObservedObject var toolPicker: ToolbarModel

    var lasso: some View {
        selectableButton(
                imageName: "lasso",
                selected: type(of: toolPicker.currentTool) == PKLassoTool.self,
                onSelect: { toolPicker.currentTool = PKLassoTool() },
                onUnSelect: { toolPicker.currentTool = PKInkingTool(.pen, color: UIColor(from: toolPicker.selectedColor)) }
        )
    }

    var eraser: some View {
        selectableButton(
                imageName: "square.righthalf.fill",
                selected: type(of: toolPicker.currentTool) == PKEraserTool.self,
                onSelect: { toolPicker.currentTool = PKEraserTool(.vector) },
                onUnSelect: { toolPicker.currentTool = PKInkingTool(.pen, color: UIColor(from: toolPicker.selectedColor)) }
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

    var body: some View {
        HStack {
            HStack {
                lasso
                eraser
                ruler
            }

            HStack {
                colorCircle(.White)
                colorCircle(.Black)
                colorCircle(.Red)
                colorCircle(.Green)
                colorCircle(.Blue)
                colorCircle(.Cyan)
                colorCircle(.Magenta)
                colorCircle(.Yellow)
            }

            HStack {
                undo
                redo
            }
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
                        .foregroundColor(Color(UIColor(from: preDarkModeConversion)))
                        .frame(width: 30, height: 30, alignment: .center)
                        .onTapGesture {
                            toolPicker.currentTool = PKInkingTool(.pen, color: UIColor(from: postDarkModeConversion))
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

class ToolbarModel: ObservableObject {
    var selectedColor: ColorAlias = .Black

    @Published var currentTool: PKTool = PKInkingTool(.pen)
    @Published var isRulerShowing: Bool = false
}


struct Toolbar_Preview: PreviewProvider {
    static let core = GlobalState()
    static let toolbar = ToolbarModel()
    static let dm = DrawingModel(core: core, meta: core.files[0])

    static var previews: some View {
        NavigationView {
            HStack {
            }
            DrawingLoader(model: dm, toolbar: toolbar)
                    .onAppear {
                        dm.originalDrawing = PKDrawing()
                        toolbar.selectedColor = .Red
                    }
        }
    }

}
