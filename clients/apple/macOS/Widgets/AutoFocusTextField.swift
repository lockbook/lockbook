import SwiftUI
import AppKit

struct AutoFocusTextField: NSViewRepresentable {
    @Binding var text: String

    let placeholder: String
    let focusRingType: NSFocusRingType
    let isBordered: Bool
    let onSubmit: () -> Void
    
    init(text: Binding<String>, placeholder: String, focusRingType: NSFocusRingType = .none, isBordered: Bool = false, onSubmit: @escaping () -> Void) {
        self._text = text
        self.placeholder = placeholder
        self.focusRingType = focusRingType
        self.isBordered = isBordered
        self.onSubmit = onSubmit
    }

    public func makeNSView(context: NSViewRepresentableContext<AutoFocusTextField>) -> NSTextField {
        let textField = NSTextField()
        textField.isBordered = isBordered
        textField.focusRingType = focusRingType
        textField.delegate = context.coordinator
        textField.placeholderString = placeholder
        
        textField.becomeFirstResponder()
        
        return textField
    }
    
    public func updateNSView(_ nsView: NSTextField, context: NSViewRepresentableContext<AutoFocusTextField>) {
        if nsView.currentEditor() == nil {
            nsView.becomeFirstResponder()
        }
    }
    
    public func makeCoordinator() -> Coordinator {
        Coordinator(self)
    }
    
    public class Coordinator: NSObject, NSTextFieldDelegate {
        var parent: AutoFocusTextField

        public init(_ parent: AutoFocusTextField) {
            self.parent = parent
        }
        
        public func control(_ control: NSControl, textView: NSTextView, doCommandBy commandSelector: Selector) -> Bool {
            if commandSelector == #selector(NSResponder.insertNewline(_:)) {
                parent.onSubmit()
                
                return true
            }
            
            return false
        }

        public func controlTextDidChange(_ obj: Notification) {
            if let textField = obj.object as? NSTextField {
                parent.text = textField.stringValue
            }
        }
    }
}
