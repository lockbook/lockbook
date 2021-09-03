import SwiftUI
import PencilKit
import SwiftLockbookCore

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
