import SwiftUI
import UIKit

struct SelectFolderTextFieldWrapper: UIViewRepresentable {
    var placeholder: String
    var onSubmit: () -> Void
        
    @StateObject var model: SelectFolderViewModel
    
    func makeCoordinator() -> Coordinator {
        Coordinator(parent: self)
    }
    
    func makeUIView(context: Context) -> SelectFolderTextField {
        let textField = SelectFolderTextField()
        textField.delegate = context.coordinator
        textField.placeholder = placeholder
        textField.returnKeyType = .done
        textField.model = model
        
        textField.becomeFirstResponder()
        
        textField.addTarget(context.coordinator, action: #selector(Coordinator.textFieldDidChange(_:)), for: .editingChanged)
        
        return textField
    }
    
    func updateUIView(_ uiView: SelectFolderTextField, context: Context) {
        uiView.text = model.searchInput
    }
        
    class Coordinator: NSObject, UITextFieldDelegate {
        var parent: SelectFolderTextFieldWrapper
        
        init(parent: SelectFolderTextFieldWrapper) {
            self.parent = parent
        }

        @objc func textFieldDidChange(_ textField: UITextField) {
            parent.model.searchInput = textField.text ?? ""
        }

        func textFieldShouldReturn(_ textField: UITextField) -> Bool {
            parent.onSubmit()
            return false
        }
    }
}

class SelectFolderTextField: UITextField {
    
    var model: SelectFolderViewModel? = nil
    
    override var keyCommands: [UIKeyCommand]? {
        let selectedUp = UIKeyCommand(input: UIKeyCommand.inputUpArrow, modifierFlags: [], action: #selector(selectedUp))
        let selectedDown = UIKeyCommand(input: UIKeyCommand.inputDownArrow, modifierFlags: [], action: #selector(selectedDown))
        let exit = UIKeyCommand(input: UIKeyCommand.inputEscape, modifierFlags: [], action: #selector(exit))
        
        selectedUp.wantsPriorityOverSystemBehavior = true
        selectedDown.wantsPriorityOverSystemBehavior = true
        exit.wantsPriorityOverSystemBehavior = true
                
        return [
            selectedUp,
            selectedDown,
            exit
        ]
    }
    
    @objc func selectedUp() {
        if let model {
            model.selected = max(model.selected - 1, 0)
        }
    }
    
    @objc func selectedDown() {
        if let model {
            model.selected = min(model.selected + 1, model.filteredFolderPaths.count - 1)
        }
    }
    
    @objc func exit() {
        model?.exit = true
    }
}
