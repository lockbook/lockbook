import SwiftUI
import SwiftLockbookCore

struct RenamingSheet: View {
    @State var noName = true
    @State var name: String = ""
    @State var introspected = false

    @EnvironmentObject var fileService: FileService
    @EnvironmentObject var sheets: SheetState

    @Environment(\.presentationMode) var presentationMode

    var body: some View {
        sheet
                .frameForMacOS()
    }

    @ViewBuilder
    var sheet: some View {
        if let meta = sheets.renamingInfo {
            if noName {
                ProgressView()
                        .onAppear {
                            name = meta.name
                            noName = false
                        }
            } else {
                VStack(alignment: .leading, spacing: 15) {
                    HStack(alignment: .center) {
                        Text("Renaming: \(meta.name)")
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
                    TextField("Renaming \(meta.name)", text: $name, onCommit: onCommit)
                            .textFieldStyle(RoundedBorderTextFieldStyle())
                            .autocapitalization(.none)
                            .introspectTextField(customize: handleCursor)
                    Spacer()
                }
                        .padding()
            }
        }
    }

    func onCommit() {
        if let meta = sheets.renamingInfo {
            if name != meta.name && name != "" {
//                fileService.renameFile(id: meta.id, name: name)
                presentationMode.wrappedValue.dismiss()
            } else {
                presentationMode.wrappedValue.dismiss()
            }
        }
    }

    #if os(iOS)
    func handleCursor(textField: UITextField) {
        if !introspected {
            introspected = true
            textField.becomeFirstResponder()
            textField.selectedTextRange = textField.textRange(from: textField.beginningOfDocument, to: textField.endOfDocument)
        }
    }
    #else
    func handleCursor(textField: NSTextField) {
        if !introspected {
            introspected = true
            textField.becomeFirstResponder()
            // TODO based on iOS
        }
    }
    #endif
}
