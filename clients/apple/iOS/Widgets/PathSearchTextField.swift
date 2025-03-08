import SwiftUI
import SwiftWorkspace
import UIKit

class PathSearchTextField: UITextField {
    
    let pathSearchModel: PathSearchViewModel
    
    init(pathSearchModel: PathSearchViewModel) {
        self.pathSearchModel = pathSearchModel

        super.init(frame: .zero)
    }
    
    required init?(coder: NSCoder) {
        fatalError("")
    }
    
    override var keyCommands: [UIKeyCommand]? {
        let selectedUp = UIKeyCommand(input: UIKeyCommand.inputUpArrow, modifierFlags: [], action: #selector(moveSelectedUp))
        let selectedDown = UIKeyCommand(input: UIKeyCommand.inputDownArrow, modifierFlags: [], action: #selector(moveSelectedDown))
        let exitPathSearch = UIKeyCommand(input: UIKeyCommand.inputEscape, modifierFlags: [], action: #selector(exitPathSearch))
        
        selectedUp.wantsPriorityOverSystemBehavior = true
        selectedDown.wantsPriorityOverSystemBehavior = true
        exitPathSearch.wantsPriorityOverSystemBehavior = true
        
        var shortcuts = [
            selectedUp,
            selectedDown,
            exitPathSearch
        ]
        
        let selectIndex1 = UIKeyCommand(input: "1", modifierFlags: [.command], action: #selector(openSelected1))
        let selectIndex2 = UIKeyCommand(input: "2", modifierFlags: [.command], action: #selector(openSelected2))
        let selectIndex3 = UIKeyCommand(input: "3", modifierFlags: [.command], action: #selector(openSelected3))
        let selectIndex4 = UIKeyCommand(input: "4", modifierFlags: [.command], action: #selector(openSelected4))
        let selectIndex5 = UIKeyCommand(input: "5", modifierFlags: [.command], action: #selector(openSelected5))
        let selectIndex6 = UIKeyCommand(input: "6", modifierFlags: [.command], action: #selector(openSelected6))
        let selectIndex7 = UIKeyCommand(input: "7", modifierFlags: [.command], action: #selector(openSelected7))
        let selectIndex8 = UIKeyCommand(input: "8", modifierFlags: [.command], action: #selector(openSelected8))
        let selectIndex9 = UIKeyCommand(input: "9", modifierFlags: [.command], action: #selector(openSelected9))
        
        selectIndex1.wantsPriorityOverSystemBehavior = true
        selectIndex2.wantsPriorityOverSystemBehavior = true
        selectIndex3.wantsPriorityOverSystemBehavior = true
        selectIndex4.wantsPriorityOverSystemBehavior = true
        selectIndex5.wantsPriorityOverSystemBehavior = true
        selectIndex6.wantsPriorityOverSystemBehavior = true
        selectIndex7.wantsPriorityOverSystemBehavior = true
        selectIndex8.wantsPriorityOverSystemBehavior = true
        selectIndex9.wantsPriorityOverSystemBehavior = true
        
        shortcuts.append(selectIndex1)
        shortcuts.append(selectIndex2)
        shortcuts.append(selectIndex3)
        shortcuts.append(selectIndex4)
        shortcuts.append(selectIndex5)
        shortcuts.append(selectIndex6)
        shortcuts.append(selectIndex7)
        shortcuts.append(selectIndex8)
        shortcuts.append(selectIndex9)
        
        return shortcuts
    }
    
    @objc func moveSelectedUp() {
        pathSearchModel.selectPreviousPath()
    }
    
    @objc func moveSelectedDown() {
        pathSearchModel.selectNextPath()
    }
    
    @objc func exitPathSearch() {
        pathSearchModel.endSearch()
    }
    
    @objc func openSelected1() {
        pathSearchModel.selected = 0
        pathSearchModel.openSelected()
    }
    
    @objc func openSelected2() {
        pathSearchModel.selected = 1
        pathSearchModel.openSelected()
    }
    
    @objc func openSelected3() {
        pathSearchModel.selected = 2
        pathSearchModel.openSelected()
    }
    
    @objc func openSelected4() {
        pathSearchModel.selected = 3
        pathSearchModel.openSelected()
    }
    
    @objc func openSelected5() {
        pathSearchModel.selected = 4
        pathSearchModel.openSelected()
    }
    
    @objc func openSelected6() {
        pathSearchModel.selected = 5
        pathSearchModel.openSelected()
    }
    
    @objc func openSelected7() {
        pathSearchModel.selected = 6
        pathSearchModel.openSelected()
    }
    
    @objc func openSelected8() {
        pathSearchModel.selected = 7
        pathSearchModel.openSelected()
    }
    
    @objc func openSelected9() {
        pathSearchModel.selected = 8
        pathSearchModel.openSelected()
    }
}

public struct PathSearchTextFieldWrapper: UIViewRepresentable {
    @State var text: String = ""
    @EnvironmentObject var pathSearchModel: PathSearchViewModel

    public func makeUIView(context: Context) -> UITextField {
        let textField = PathSearchTextField(pathSearchModel: pathSearchModel)
        textField.delegate = context.coordinator
        textField.becomeFirstResponder()
        textField.autocapitalizationType = .none
        textField.autocorrectionType = .no

        return textField
    }

    public func updateUIView(_ uiView: UITextField, context: Context) {
        uiView.text = text
        
        DispatchQueue.main.async {
            if pathSearchModel.isShown {
                pathSearchModel.input = text
                pathSearchModel.search()
            }
        }
        
    }

    public func makeCoordinator() -> Coordinator {
        Coordinator(self)
    }

    public class Coordinator: NSObject, UITextFieldDelegate {
        var parent: PathSearchTextFieldWrapper

        public init(_ parent: PathSearchTextFieldWrapper) {
            self.parent = parent
        }

        public func textFieldDidChangeSelection(_ textField: UITextField) {
            parent.text = textField.text ?? ""
        }

        public func textFieldShouldReturn(_ textField: UITextField) -> Bool {
            parent.pathSearchModel.openSelected()
            return false
        }
    }
}
