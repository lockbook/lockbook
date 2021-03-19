import SwiftUI
import SwiftLockbookCore

struct FileCell: View {
    let meta: FileMetadata
    
    var body: some View {
        HStack {
            VStack(alignment: .leading, spacing: 5) {
                Text(meta.name)
                    .font(.title3)
                HStack {
                    Image(systemName: meta.fileType == .Folder ? "folder" : "doc")
                        .foregroundColor(meta.fileType == .Folder ? .blue : .secondary)
                    Text(intEpochToString(epoch: meta.contentVersion))
                        .foregroundColor(.secondary)
                    
                }.font(.footnote)
            }
            .padding(.vertical, 5)
            Spacer()
            
            if meta.fileType == .Folder {
                Image(systemName: "chevron.right")
                    .padding(.trailing, 10)
                    .foregroundColor(.secondary)
            }
        }
        .contentShape(Rectangle()) /// https://stackoverflow.com/questions/57258371/swiftui-increase-tap-drag-area-for-user-interaction
    }
}

struct SyntheticFileCell: View {
    let parent: FileMetadata
    let type: FileType
    @Binding var nameField: String
    @Binding var fileExtension: String
    let onCommit: () -> Void
    let onCancel: () -> Void
    
    var newWhat: String {
        if fileExtension == ".draw" && type == .Document {
            return "Drawing"
        } else {
            return type.rawValue
        }
    }
    
    var body: some View {
        HStack {
            VStack(alignment: .leading, spacing: 5) {
                HStack {
                    TextField(type == .Folder ? "folder name" : "document name", text: $nameField,
                              onCommit: onCommit)
                        .autocapitalization(.none)
                        .font(.title3)
                    if type == .Document {
                        TextField("File Extension", text: $fileExtension,
                                  onCommit: onCommit)
                            .autocapitalization(.none)
                            .font(.title3)
                            .frame(width: 50)
                    }
                }
                HStack {
                    Image(systemName: type == .Folder ? "folder" : "doc")
                        .foregroundColor(type == .Folder ? .blue : .secondary)
                    Text("New \(newWhat) in \(parent.name)")
                        .foregroundColor(.gray)
                }.font(.footnote)
            }.padding(.vertical, 5)
            
            Button(action: onCancel) {
                Image(systemName: "xmark")
                    .foregroundColor(.red)
            }.padding(.trailing, 10)
        }
        
    }
}

struct FileCell_Previews: PreviewProvider {
    static var previews: some View {
        Group {
            FileCell(meta: GlobalState().files[0])
            SyntheticFileCell(parent: GlobalState().files[0], type: .Document, nameField: .constant(""), fileExtension: .constant(".md"), onCommit: {}, onCancel: {})
            
            SyntheticFileCell(parent: GlobalState().files[0], type: .Document, nameField: .constant(""), fileExtension: .constant(".text"), onCommit: {}, onCancel: {})
            
            SyntheticFileCell(parent: GlobalState().files[0], type: .Document, nameField: .constant(""), fileExtension: .constant(".draw"), onCommit: {}, onCancel: {})
            
            SyntheticFileCell(parent: GlobalState().files[0], type: .Folder, nameField: .constant(""), fileExtension: .constant(".md"), onCommit: {}, onCancel: {})
            
        }
        .previewLayout(.fixed(width: 300, height: 50))
    }
}
