import Foundation
import SwiftUI
import SwiftWorkspace

struct RenameFileSheet: View {
    let info: RenamingFileInfo
    
    @State var newName: String = ""
    @State var error: String? = nil
    
    @Environment(\.dismiss) private var dismiss
    
    init(info: RenamingFileInfo) {
        self.info = info
        self._newName = State(initialValue: info.name)
    }
    
    var body: some View {
        VStack(spacing: 10) {
            HStack {
                Text("Rename File")
                    .bold()
                
                Spacer()
            }
            
            HStack {
                Text("Parent Folder:")
                    .font(.callout)
                
                Text(info.parentPath)
                    .lineLimit(2)
                    .font(.system(.callout, design: .monospaced))
                
                Spacer()
            }
            
            RenameFileTextFieldWrapper(placeholder: "File name", onSubmit: {
                guard info.name != newName else {
                    dismiss()
                    return
                }
                
                renameFile()
            }, newName: $newName)
                                    
            if let error = error {
                HStack {
                    Text(error)
                        .foregroundStyle(.red)
                        .fontWeight(.bold)
                        .lineLimit(2, reservesSpace: false)
                    
                    Spacer()
                }
            }
                                    
            Button {
                renameFile()
            } label: {
                Text("Rename")
                    .frame(maxWidth: .infinity)
            }
            .buttonStyle(.bordered)
            .disabled(info.name == newName)

        }
        .padding(.horizontal)
        .padding(.top, 3)
    }
    
    func renameFile() {
        let res = DI.files.renameFileSync(id: info.id, name: newName)
        
        switch res {
        case .some(let errMsg):
            error = errMsg
        case .none:
            DI.workspace.fileOpCompleted = .Rename(id: info.id, newName: newName)
            dismiss()
        }
    }
}

#if os(iOS)
struct RenameFileTextFieldWrapper: UIViewRepresentable {
    var placeholder: String
    var onSubmit: () -> Void
        
    @Binding var newName: String
        
    func makeUIView(context: Context) -> UITextField {
        let textField = UITextField()
        textField.delegate = context.coordinator
        textField.placeholder = placeholder
        textField.returnKeyType = .done
        textField.borderStyle = .roundedRect
        textField.text = newName
        textField.setContentHuggingPriority(.defaultHigh, for: .vertical)
        
        textField.becomeFirstResponder()
        
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.1, execute: {
            if let extIndex = newName.lastIndex(of: ".") {
                let startPos = textField.beginningOfDocument
                if let endPos = textField.position(from: startPos, offset: newName.distance(from: newName.startIndex, to: extIndex)) {
                    textField.selectedTextRange = textField.textRange(from: startPos, to: endPos)
                }
            }
        })
                            
        textField.addTarget(context.coordinator, action: #selector(Coordinator.textFieldDidChange(_:)), for: .editingChanged)
        
        return textField
    }
    
    func updateUIView(_ uiView: UITextField, context: Context) {}
        
    func makeCoordinator() -> Coordinator {
        Coordinator(parent: self)
    }
    
    class Coordinator: NSObject, UITextFieldDelegate {
        var parent: RenameFileTextFieldWrapper
        private var didSetInitialSelection = false
        
        init(parent: RenameFileTextFieldWrapper) {
            self.parent = parent
        }

        @objc func textFieldDidChange(_ textField: UITextField) {
            parent.newName = textField.text ?? ""
        }

        func textFieldShouldReturn(_ textField: UITextField) -> Bool {
            parent.onSubmit()
            return false
        }
    }
}
#else
struct RenameFileTextFieldWrapper: NSViewRepresentable {
    var placeholder: String
    var onSubmit: () -> Void
    
    @Binding var newName: String
    
    public func makeNSView(context: NSViewRepresentableContext<RenameFileTextFieldWrapper>) -> NSTextField {
        let textField = NSTextField()
        textField.isBordered = false
        textField.focusRingType = .none
        textField.delegate = context.coordinator
        textField.placeholderString = placeholder
        textField.stringValue = newName
        
        textField.becomeFirstResponder()
        
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.00001, execute: {
            setInitialSelection(textField: textField)
        })
        
        return textField
    }
    
    public func updateNSView(_ nsView: NSTextField, context: NSViewRepresentableContext<RenameFileTextFieldWrapper>) {}
    
    func setInitialSelection(textField: NSTextField) {
        if let editor = textField.currentEditor(),
           let extIndex = newName.lastIndex(of: ".") {
            editor.selectedRange = NSRange(newName.startIndex..<extIndex, in: newName)
        }
    }
    
    public func makeCoordinator() -> Coordinator {
        Coordinator(self)
    }
    
    public class Coordinator: NSObject, NSTextFieldDelegate {
        var parent: RenameFileTextFieldWrapper

        public init(_ parent: RenameFileTextFieldWrapper) {
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
                parent.newName = textField.stringValue
            }
        }
    }
}
#endif
