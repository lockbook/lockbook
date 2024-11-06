import Foundation
import SwiftUI

struct CreateFolderSheet: View {
    let info: CreatingFolderInfo
    
    @State var name: String = ""
    @State var error: String? = nil
    
    @Environment(\.dismiss) private var dismiss
    
    var body: some View {
        VStack(spacing: 10) {
            HStack {
                Text("New Folder")
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
            
            CreateFolderTextFieldWrapper(placeholder: "Folder name", onSubmit: {
                createFolder()
            }, name: $name)
                                    
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
                createFolder()
            } label: {
                Text("Create")
                    .frame(maxWidth: .infinity)
            }
            .buttonStyle(.bordered)
            .disabled(name.isEmpty)
        }
        .padding(.horizontal)
        .padding(.top, 3)
    }
    
    func createFolder() {
        let res = DI.files.createFolderSync(name: name, maybeParent: info.maybeParent)
        
        switch res {
        case .some(let errMsg):
            error = errMsg
        case .none:
            dismiss()
        }
    }
}

#if os(iOS)
import UIKit

struct CreateFolderTextFieldWrapper: UIViewRepresentable {
    var placeholder: String
    var onSubmit: () -> Void
        
    @Binding var name: String

    func makeCoordinator() -> Coordinator {
        Coordinator(parent: self)
    }
    
    func makeUIView(context: Context) -> UITextField {
        let textField = UITextField()
        textField.delegate = context.coordinator
        textField.placeholder = placeholder
        textField.returnKeyType = .done
        textField.borderStyle = .roundedRect
        textField.text = name
        textField.setContentHuggingPriority(.defaultHigh, for: .vertical)
        
        textField.becomeFirstResponder()
        
        textField.addTarget(context.coordinator, action: #selector(Coordinator.textFieldDidChange(_:)), for: .editingChanged)
        
        return textField
    }
    
    func updateUIView(_ uiView: UITextField, context: Context) {
        uiView.text = name
    }
        
    class Coordinator: NSObject, UITextFieldDelegate {
        var parent: CreateFolderTextFieldWrapper
        
        init(parent: CreateFolderTextFieldWrapper) {
            self.parent = parent
        }

        @objc func textFieldDidChange(_ textField: UITextField) {
            parent.name = textField.text ?? ""
        }

        func textFieldShouldReturn(_ textField: UITextField) -> Bool {
            parent.onSubmit()
            return false
        }
    }
}
#else
struct CreateFolderTextFieldWrapper: NSViewRepresentable {
    var placeholder: String
    var onSubmit: () -> Void
    
    @Binding var name: String
    
    public func makeNSView(context: NSViewRepresentableContext<CreateFolderTextFieldWrapper>) -> NSTextField {
        let textField = NSTextField()
        textField.isBordered = false
        textField.focusRingType = .none
        textField.delegate = context.coordinator
        textField.placeholderString = placeholder
        
        textField.becomeFirstResponder()
        
        return textField
    }
    
    public func updateNSView(_ nsView: NSTextField, context: NSViewRepresentableContext<CreateFolderTextFieldWrapper>) {
        if nsView.currentEditor() == nil {
            nsView.becomeFirstResponder()
        }
    }
    
    public func makeCoordinator() -> Coordinator {
        Coordinator(self)
    }
    
    public class Coordinator: NSObject, NSTextFieldDelegate {
        var parent: CreateFolderTextFieldWrapper

        public init(_ parent: CreateFolderTextFieldWrapper) {
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
                parent.name = textField.stringValue
            }
        }
    }
}
#endif

struct CreateFolderSheet_Previews: PreviewProvider {
    static var previews: some View {
        Rectangle()
            .foregroundStyle(.white)
            .sheet(isPresented: Binding.constant(true), content: {
                CreateFolderSheet(info: CreatingFolderInfo(parentPath: "Apple", maybeParent: nil))
                    .presentationDetents([.height(150)])
                    .presentationDragIndicator(.visible)
            })
    }
}
