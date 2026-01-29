import SwiftUI
import UIKit

struct AutoFocusTextField: UIViewRepresentable {
    @State var selectedName: Bool = false
    @Binding var text: String

    let placeholder: String
    let returnKeyType: UIReturnKeyType
    let borderStyle: UITextField.BorderStyle
    let onSubmit: () -> Void
    let autocorrectionType: UITextAutocorrectionType
    
    init(text: Binding<String>, placeholder: String, returnKeyType: UIReturnKeyType = .done, borderStyle: UITextField.BorderStyle = .roundedRect, autocorrect: Bool = false, onSubmit: @escaping () -> Void) {
        self._text = text
        
        self.placeholder = placeholder
        self.returnKeyType = returnKeyType
        self.borderStyle = borderStyle
        self.autocorrectionType = autocorrect ? .yes : .no
        self.onSubmit = onSubmit
    }
    
    func makeCoordinator() -> Coordinator {
        Coordinator(parent: self)
    }
    
    func makeUIView(context: Context) -> UITextField {
        let textField = UITextField()
        textField.delegate = context.coordinator
        textField.placeholder = placeholder
        textField.returnKeyType = returnKeyType
        textField.borderStyle = borderStyle
        textField.text = text
        textField.autocorrectionType = autocorrectionType
        textField.setContentHuggingPriority(.defaultHigh, for: .vertical)
        
        textField.becomeFirstResponder()
        
        textField.addTarget(context.coordinator, action: #selector(Coordinator.textFieldDidChange(_:)), for: .editingChanged)
        
        return textField
    }
    
    func updateUIView(_ uiView: UITextField, context: Context) {
        uiView.text = text
        
        guard let baseName = (uiView.text as? NSString)?.deletingPathExtension else { return }
        
        if !selectedName,
           let start = uiView.position(from: uiView.beginningOfDocument, offset: 0),
           let end = uiView.position(from: start, offset: baseName.count) {
            DispatchQueue.main.async {
                self.selectedName = true
            }
            
            uiView.selectedTextRange = uiView.textRange(from: start, to: end)
        }
    }
        
    class Coordinator: NSObject, UITextFieldDelegate {
        var parent: AutoFocusTextField
        
        init(parent: AutoFocusTextField) {
            self.parent = parent
        }

        @objc func textFieldDidChange(_ textField: UITextField) {
            parent.text = textField.text ?? ""
        }

        func textFieldShouldReturn(_ textField: UITextField) -> Bool {
            parent.onSubmit()
            return false
        }
    }
}
