import SwiftUI
import SwiftLockbookCore

enum ClientFileTypes {
    case Document
    case Folder
    case Drawing
    
}

struct NewFileSheet: View {
    let parent: ClientFileMetadata
    
    @ObservedObject var core: GlobalState
    
    @State var selected: ClientFileTypes = .Document
    @State var name: String = ".md"
    
    var onSuccess: (_: ClientFileMetadata) -> Void
    
    var body: some View {
        VStack (alignment: .leading, spacing: 15){
            HStack (alignment: .center) {
                Text("Create")
                    .bold()
                    .font(.title)
                Spacer()
                Button(action: {print("hi")}) {
                    Image(systemName: "xmark.circle.fill")
                        .foregroundColor(.gray)
                        .imageScale(.large)
                        .frame(width: 50, height: 50, alignment: .center)
                }
            }
            HStack {
                Text("Inside:")
                Text(parent.name + "/")
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
            
            Spacer()
        }.padding()
    }
    
    func handleCursor(textField: UITextField) {
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
    
    func selectionChanged(selection: ClientFileTypes) {
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
        switch core.api.createFile(name: name, dirId: parent.id, isFolder: selected == .Folder) {
        case .success(let newMeta):
            core.updateFiles()
            core.checkForLocalWork()
            onSuccess(newMeta)
        case .failure(let err):
            core.handleError(err)
        }
    }
}
