import SwiftUI
import SwiftLockbookCore

enum ClientFileTypes {
    case Document
    case Folder
    case Drawing
    
}

struct NewFileSheet: View {
    
    @EnvironmentObject var files: FileService
    @EnvironmentObject var status: StatusService
    @EnvironmentObject var errorService: UnexpectedErrorService
    
    @Environment(\.presentationMode) var presentationMode
    
    let parent: DecryptedFileMetadata
    
    @State var selected: ClientFileTypes = .Document
    @State var name: String = ".md"
    @State var errors: String = ""
    @State var introspected = false
    
    var onSuccess: (_: DecryptedFileMetadata) -> Void
    
    var body: some View {
        VStack (alignment: .leading, spacing: 15){
            HStack (alignment: .center) {
                Text("Create")
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
            HStack {
                Text("Inside:")
                Text(parent.decryptedName + "/")
                    .font(.system(.body, design: .monospaced))
            }
            Picker(selection: $selected, label: Text(""), content: {
                Text("Document").tag(ClientFileTypes.Document)
                Text("Drawing").tag(ClientFileTypes.Drawing)
                Text("Folder").tag(ClientFileTypes.Folder)
            }).pickerStyle(SegmentedPickerStyle())
                .onChange(of: selected, perform: selectionChanged)
            
            TextField("Choose a username", text: $name, onCommit: onCommit)
                .textFieldStyle(RoundedBorderTextFieldStyle())
                .autocapitalization(.none)
                .tag("FileNameView")
                .introspectTextField(customize: handleCursor)
            
            if errors != "" {
                Text(errors)
                    .foregroundColor(.red)
                    .bold()
            }
            
            Spacer()
        }.padding()
    }
    
    func handleCursor(textField: UITextField) {
        if !introspected {
            introspected = true
            textField.becomeFirstResponder()
            
            switch selected {
            case .Document:
                if let upperBound = textField.position(from: textField.endOfDocument, offset: -3) {
                    textField.selectedTextRange = textField.textRange(from: textField.beginningOfDocument, to: upperBound)
                }
            case .Drawing:
                if let upperBound = textField.position(from: textField.endOfDocument, offset: -5) {
                    textField.selectedTextRange = textField.textRange(from: textField.beginningOfDocument, to: upperBound)
                }
            case .Folder:
                break
            }
        }
    }
    
    func selectionChanged(selection: ClientFileTypes) {
        introspected = false
        switch selection {
        case .Document:
            name = ".md"
        case .Drawing:
            name = ".draw"
        case .Folder:
            name = ""
        }
    }
    
    func onCommit() {
        switch DI.core.createFile(name: name, dirId: parent.id, isFolder: selected == .Folder) {
        case .success(let newMeta):
            files.refresh()
            status.checkForLocalWork()
            onSuccess(newMeta)
        case .failure(let err):
            switch err.kind {
            case .UiError(let uiError):
                switch uiError {
                case .FileNameContainsSlash:
                    errors = "File names cannot contain slashes"
                case .FileNameEmpty:
                    errors = "File name cannot be empty"
                case .FileNameNotAvailable:
                    errors = "A file with that name exists in this folder already"
                default:
                    errorService.handleError(err)
                }
                break;
            case .Unexpected:
                errorService.handleError(err)
            }
        }
    }
}
