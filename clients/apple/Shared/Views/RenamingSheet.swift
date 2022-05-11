import SwiftUI
import SwiftLockbookCore

struct RenamingSheet: View {
    let meta: DecryptedFileMetadata?
    @State var noName = true
    @State var name: String = ""
    @State var introspected = false
    
    @EnvironmentObject var fileService: FileService
    
    @Environment(\.presentationMode) var presentationMode
    
    var body: some View {
        if let meta = meta {
            if noName {
                ProgressView()
                    .onAppear {
                        name = meta.decryptedName
                        noName = false
                    }
            } else {
                VStack(alignment: .leading, spacing: 15) {
                    HStack (alignment: .center) {
                        Text("Renaming: \(meta.decryptedName)")
                            .bold()
                            .font(.title)
                        Spacer()
                        Button(action: { presentationMode.wrappedValue.dismiss() }) {
                            Image(systemName: "xmark.circle.fill")
                                .foregroundColor(.gray)
                                .imageScale(.large)
                                .frame(width: 50, height: 50, alignment: .center)
                        }
                    }
                    TextField("Renaming \(meta.decryptedName)", text: $name, onCommit: onCommit)
                        .textFieldStyle(RoundedBorderTextFieldStyle())
                        .autocapitalization(.none)
                        .introspectTextField(customize: handleCursor)
                    Spacer()
                }.padding()
            }
        }
    }
    
    func onCommit() {
        if let meta = meta {
            if name != meta.decryptedName && name != "" {
                fileService.renameFile(id: meta.id, name: name)
                presentationMode.wrappedValue.dismiss()
            } else {
                presentationMode.wrappedValue.dismiss()
            }
        }
    }
    
    func handleCursor(textField: UITextField) {
        if !introspected {
            introspected = true
            textField.becomeFirstResponder()
            textField.selectedTextRange = textField.textRange(from: textField.beginningOfDocument, to: textField.endOfDocument)
        }
    }
}
