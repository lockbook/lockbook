import SwiftUI
import SwiftWorkspace

public class PathSearchTextField: NSTextField {
    let pathSearchModel: PathSearchViewModel
    
    init(pathSearchModel: PathSearchViewModel) {
        self.pathSearchModel = pathSearchModel
        
        super.init(frame: .zero)
    }
    
    required init(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }
    
    public override func performKeyEquivalent(with event: NSEvent) -> Bool {
        switch event.keyCode {
        case 126: // up arrow
            pathSearchModel.selectPreviousPath()
            return true
        case 125: // down arrow
            pathSearchModel.selectNextPath()
            return true
        case 36: // return
            pathSearchModel.openSelected()
            return true
        default:
            if event.modifierFlags.contains(.command) { // command + num (1-9)
                if event.keyCode == 18  {
                    pathSearchModel.selected = 0
                    pathSearchModel.openSelected()
                } else if event.keyCode == 19 {
                    pathSearchModel.selected = 1
                    pathSearchModel.openSelected()
                } else if event.keyCode == 20 {
                    pathSearchModel.selected = 2
                    pathSearchModel.openSelected()
                } else if event.keyCode == 21 {
                    pathSearchModel.selected = 3
                    pathSearchModel.openSelected()
                } else if event.keyCode == 23 {
                    pathSearchModel.selected = 4
                    pathSearchModel.openSelected()
                } else if event.keyCode == 22 {
                    pathSearchModel.selected = 5
                    pathSearchModel.openSelected()
                } else if event.keyCode == 26 {
                    pathSearchModel.selected = 6
                    pathSearchModel.openSelected()
                } else if event.keyCode == 28 {
                    pathSearchModel.selected = 7
                    pathSearchModel.openSelected()
                } else if event.keyCode == 25 {
                    pathSearchModel.selected = 8
                    pathSearchModel.openSelected()
                } else {
                    return super.performKeyEquivalent(with: event)
                }
                
                return true
            }
            
            
            return super.performKeyEquivalent(with: event)
        }
    }
    
    public override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        guard let window = self.window else { return }
        window.makeFirstResponder(self)
    }
    
    public override func cancelOperation(_ sender: Any?) {
        pathSearchModel.endSearch()
    }
}

public struct PathSearchTextFieldWrapper: NSViewRepresentable {
    @EnvironmentObject var pathSearchModel: PathSearchViewModel
            
    public func makeNSView(context: NSViewRepresentableContext<PathSearchTextFieldWrapper>) -> PathSearchTextField {
        let textField = PathSearchTextField(pathSearchModel: pathSearchModel)
        textField.isBordered = false
        textField.focusRingType = .none
        textField.delegate = context.coordinator
        textField.font = .systemFont(ofSize: 15)
        textField.backgroundColor = nil
        
        return textField
    }
    
    public func updateNSView(_ nsView: PathSearchTextField, context: NSViewRepresentableContext<PathSearchTextFieldWrapper>) {
        
    }
    
    public func makeCoordinator() -> PathSearchTextFieldDelegate {
        PathSearchTextFieldDelegate(self)
    }
    
    public class PathSearchTextFieldDelegate: NSObject, NSTextFieldDelegate {
        var parent: PathSearchTextFieldWrapper

        public init(_ parent: PathSearchTextFieldWrapper) {
            self.parent = parent
        }

        public func controlTextDidChange(_ obj: Notification) {
            if let textField = obj.object as? NSTextField {
                DispatchQueue.main.async {
                    if self.parent.pathSearchModel.isShown {
                        self.parent.pathSearchModel.input = textField.stringValue
                        self.parent.pathSearchModel.search()
                    }
                }
            }
        }
    }
}
