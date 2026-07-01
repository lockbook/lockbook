import AppKit
import SwiftUI

struct AutoFocusTextField: NSViewRepresentable {
    @State var selectedName: Bool = false
    @Binding var text: String

    let placeholder: String
    let focusRingType: NSFocusRingType
    let isBordered: Bool
    let onSubmit: () -> Void

    init(text: Binding<String>, placeholder: String, focusRingType: NSFocusRingType = .none, isBordered: Bool = false, onSubmit: @escaping () -> Void) {
        _text = text
        self.placeholder = placeholder
        self.focusRingType = focusRingType
        self.isBordered = isBordered
        self.onSubmit = onSubmit
    }

    func makeNSView(context: NSViewRepresentableContext<AutoFocusTextField>) -> NSTextField {
        let textField = NSTextField()
        textField.isBordered = isBordered
        textField.focusRingType = focusRingType
        textField.delegate = context.coordinator
        textField.placeholderString = placeholder
        textField.drawsBackground = false
        textField.isBezeled = isBordered
        textField.wantsLayer = true
        textField.layer?.cornerRadius = 4
        textField.stringValue = text

        textField.becomeFirstResponder()

        return textField
    }

    func updateNSView(_ nsView: NSTextField, context _: NSViewRepresentableContext<AutoFocusTextField>) {
        if nsView.currentEditor() == nil {
            nsView.becomeFirstResponder()
        }

        if let editor = nsView.currentEditor(), !selectedName {
            DispatchQueue.main.async {
                selectedName = true
            }

            let baseName = (nsView.stringValue as NSString).deletingPathExtension
            editor.selectedRange = NSRange(location: 0, length: baseName.count)
        }
    }

    func makeCoordinator() -> Coordinator {
        Coordinator(self)
    }

    class Coordinator: NSObject, NSTextFieldDelegate {
        var parent: AutoFocusTextField

        init(_ parent: AutoFocusTextField) {
            self.parent = parent
        }

        func control(_: NSControl, textView _: NSTextView, doCommandBy commandSelector: Selector) -> Bool {
            if commandSelector == #selector(NSResponder.insertNewline(_:)) {
                parent.onSubmit()

                return true
            }

            return false
        }

        func controlTextDidChange(_ obj: Notification) {
            if let textField = obj.object as? NSTextField {
                parent.text = textField.stringValue
            }
        }
    }
}
