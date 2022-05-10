import SwiftUI
import SwiftLockbookCore
import Introspect

struct FileCell: View {
    let meta: DecryptedFileMetadata
    @State var renaming = false
    @State var newName: String = ""
    @State var moving = false
    
    @Binding var selection: DecryptedFileMetadata?
    
    @EnvironmentObject var fileService: FileService
    @EnvironmentObject var account: AccountService
    
    var body: some View {
        renamableView
            .sheet(isPresented: $moving) { MoveSheet(meta: meta) }
    }
    
    @ViewBuilder
    var renamableView: some View {
        if renaming {
            renamingView
        } else {
            realView
                .contextMenu(menuItems: {
                    Button(action: handleDelete) {
                        Label("Delete", systemImage: "trash.fill")
                    }
                    Button(action: {
                        moving = true
                    }, label: {
                        Label("Move", systemImage: "folder")
                    })
                    Button(action: {
                        renaming = true
                        newName = meta.decryptedName
                    }, label: {
                        Label("Rename", systemImage: "pencil")
                    })
                })
        }
    }
    
    @ViewBuilder
    var realView: some View {
        if meta.fileType == .Folder {
            NavigationLink(
                destination: FileListView(currentFolder: meta, account: account.account!), tag: meta, selection: $selection) {
                    RealFileCell(meta: meta)
                }.isDetailLink(false)
        } else {
            NavigationLink(destination: DocumentView(meta: meta), tag: meta, selection: $selection) {
                RealFileCell(meta: meta)
            }
        }
    }
    
    var renamingView: some View {
        HStack {
            VStack(alignment: .leading, spacing: 5) {
                TextField(meta.fileType == .Folder ? "folder name" : "document name", text: $newName, onCommit: onCommit)
                    .autocapitalization(.none)
                    .font(.title3)
                    .introspectTextField(customize: { textField in
                        textField.becomeFirstResponder()
                        textField.selectedTextRange = textField.textRange(from: textField.beginningOfDocument, to: textField.endOfDocument)
                    })
                HStack {
                    Image(systemName: meta.fileType == .Folder ? "folder" : "doc")
                        .foregroundColor(meta.fileType == .Folder ? .blue : .secondary)
                    Text("Renaming \(meta.decryptedName)")
                        .foregroundColor(.gray)
                }.font(.footnote)
            }
            
            Button(action: onCancel) {
                Image(systemName: "xmark")
                    .foregroundColor(.red)
            }.padding(.trailing, 10)
        }.padding(.vertical, 5)
    }
    
    func onCommit() {
        fileService.renameFile(id: meta.id, name: newName)
        renaming = false
    }
    
    func onCancel() {
        renaming = false
        newName = ""
    }
    
    func handleDelete() {
        self.fileService.deleteFile(id: meta.id)
        selection = .none
    }
}

struct RealFileCell: View {
    let meta: DecryptedFileMetadata
    
    var body: some View {
        VStack(alignment: .leading, spacing: 5) {
            Text(meta.decryptedName)
                .font(.title3)
            HStack {
                Image(systemName: meta.fileType == .Folder ? "folder" : "doc")
                    .foregroundColor(meta.fileType == .Folder ? .blue : .secondary)
                Text(intEpochToString(epoch: max(meta.metadataVersion, meta.contentVersion)))
                    .foregroundColor(.secondary)
                
            }.font(.footnote)
        }
        .padding(.vertical, 5)
        
        .contentShape(Rectangle()) /// https://stackoverflow.com/questions/57258371/swiftui-increase-tap-drag-area-for-user-interaction
    }
}

struct RenamingFileCell: View {
    let parent: DecryptedFileMetadata
    let type: FileType
    @State var name: String
    let onCommit: () -> Void
    let onCancel: () -> Void
    let renaming: Bool
    
    var newWhat: String {
        if name.hasSuffix(".draw") && type == .Document {
            return "Drawing"
        } else {
            return type.rawValue
        }
    }
    
    var body: some View {
        HStack {
            VStack(alignment: .leading, spacing: 5) {
                TextField(type == .Folder ? "folder name" : "document name", text: $name, onCommit: onCommit)
                    .autocapitalization(.none)
                    .font(.title3)
                    .introspectTextField(customize: { textField in
                        textField.becomeFirstResponder()
                        textField.selectedTextRange = textField.textRange(from: textField.beginningOfDocument, to: textField.endOfDocument)
                    })
                HStack {
                    Image(systemName: type == .Folder ? "folder" : "doc")
                        .foregroundColor(type == .Folder ? .blue : .secondary)
                    Text(renaming ? "Renaming \(parent.decryptedName)" : "New \(newWhat) in \(parent.decryptedName)")
                        .foregroundColor(.gray)
                }.font(.footnote)
            }
            
            Button(action: onCancel) {
                Image(systemName: "xmark")
                    .foregroundColor(.red)
            }.padding(.trailing, 10)
        }.padding(.vertical, 5)
    }
}

//struct FileCell_Previews: PreviewProvider {
//    static var previews: some View {
//        Group {
//            RealFileCell(meta: Mock.files.files[0])
//            RenamingFileCell(parent: Mock.files.files[0], type: .Document, name: .constant(""), onCommit: {}, onCancel: {}, renaming: false)
//
//            RenamingFileCell(parent: Mock.files.files[0], type: .Document, name: .constant(""), onCommit: {}, onCancel: {}, renaming: false)
//
//            RenamingFileCell(parent: Mock.files.files[0], type: .Document, name: .constant(""), onCommit: {}, onCancel: {}, renaming: false)
//
//            RenamingFileCell(parent: Mock.files.files[0], type: .Folder, name: .constant(""), onCommit: {}, onCancel: {}, renaming: false)
//
//        }
//        .mockDI()
//        .previewLayout(.fixed(width: 300, height: 50))
//    }
//}
