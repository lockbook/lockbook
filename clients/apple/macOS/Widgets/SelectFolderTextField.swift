import SwiftUI

struct SelectFolderTextFieldWrapper: NSViewRepresentable {
    var placeholder: String
    var onSubmit: () -> Void

    let model: SelectFolderViewModel
    
    public func makeNSView(context: NSViewRepresentableContext<SelectFolderTextFieldWrapper>) -> SelectFolderTextField {
        let textField = SelectFolderTextField()

        textField.isBordered = false
        textField.focusRingType = .none
        textField.delegate = context.coordinator
        textField.placeholderString = placeholder
        textField.onSubmit = onSubmit
        textField.model = model
        textField.drawsBackground = true
        
        textField.wantsLayer = true
        textField.layer?.cornerRadius = 4

        textField.becomeFirstResponder()
        
        return textField
    }
    
    public func updateNSView(_ nsView: SelectFolderTextField, context: NSViewRepresentableContext<SelectFolderTextFieldWrapper>) {
        if nsView.currentEditor() == nil {
            nsView.becomeFirstResponder()
        }
    }
    
    public func makeCoordinator() -> SelectFolderTextFieldDelegate {
        SelectFolderTextFieldDelegate(self)
    }
    
    public class SelectFolderTextFieldDelegate: NSObject, NSTextFieldDelegate {
        var parent: SelectFolderTextFieldWrapper

        public init(_ parent: SelectFolderTextFieldWrapper) {
            self.parent = parent
        }

        public func controlTextDidChange(_ obj: Notification) {
            if let textField = obj.object as? NSTextField {
                parent.model.searchInput = textField.stringValue
            }
        }
    }
}

class SelectFolderTextField: NSTextField {
    
    var model: SelectFolderViewModel? = nil
    var onSubmit: (() -> Void)? = nil
    
    public override func performKeyEquivalent(with event: NSEvent) -> Bool {
        switch event.keyCode {
        case 126: // up arrow
            selectedUp()
            return true
        case 125: // down arrow
            selectedDown()
            return true
        case 36: // return
            onSubmit?()
            return true
        default:
            return super.performKeyEquivalent(with: event)
        }
    }
    
    func selectedUp() {
        if let model {
            model.selected = max(model.selected - 1, 0)
        }
    }
    
    func selectedDown() {
        if let model {
            model.selected = min(model.selected + 1, model.filteredFolderPaths.count - 1)
        }
    }
    
    func exit() {
        model?.exit = true
    }
}
