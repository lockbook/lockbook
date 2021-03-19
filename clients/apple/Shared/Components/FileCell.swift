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
    @Binding var name: String
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
                ZStack {
                    TextField(type == .Folder ? "folder name" : "document name", text: $name, onCommit: onCommit)
                        .autocapitalization(.none)
                        .font(.title3)
                }
                HStack {
                    Image(systemName: type == .Folder ? "folder" : "doc")
                        .foregroundColor(type == .Folder ? .blue : .secondary)
                    Text(renaming ? "Renaming \(parent.name)" : "New \(newWhat) in \(parent.name)")
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

struct FileCell_Previews: PreviewProvider {
    static var previews: some View {
        Group {
            FileCell(meta: GlobalState().files[0])
            SyntheticFileCell(parent: GlobalState().files[0], type: .Document, name: .constant(""), onCommit: {}, onCancel: {}, renaming: false)
            
            SyntheticFileCell(parent: GlobalState().files[0], type: .Document, name: .constant(""), onCommit: {}, onCancel: {}, renaming: false)
            
            SyntheticFileCell(parent: GlobalState().files[0], type: .Document, name: .constant(""), onCommit: {}, onCancel: {}, renaming: false)
            
            SyntheticFileCell(parent: GlobalState().files[0], type: .Folder, name: .constant(""), onCommit: {}, onCancel: {}, renaming: false)
            
        }
        .previewLayout(.fixed(width: 300, height: 50))
    }
}
