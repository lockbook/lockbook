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
    }
}

struct SyntheticFileCell: View {
    let params: (FileMetadata, FileType)
    @Binding var nameField: String
    let onCreate: () -> Void
    let onCancel: () -> Void
    
    var body: some View {
        HStack {
            VStack(alignment: .leading) {
                TextField(params.1 == .Folder ? "folder name" : "document name", text: $nameField,
                          onCommit: onCreate)
                    .autocapitalization(.none)
                    .font(.title3)
                HStack {
                    Image(systemName: params.1 == .Folder ? "folder" : "doc")
                        .foregroundColor(params.1 == .Folder ? .blue : .secondary)
                    Text("New \(params.1.rawValue) in \(params.0.name)")
                        .foregroundColor(.gray)
                    
                }.font(.footnote)
            }
            Button(action: onCancel) {
                Image(systemName: "xmark")
                    .foregroundColor(.red)
            }
        }
        
    }
}

struct FileCell_Previews: PreviewProvider {
    static var previews: some View {
        Group {
            FileCell(meta: Core().files[0])
            SyntheticFileCell(params: (Core().files[0], .Document), nameField: .constant(""), onCreate: {}, onCancel: {})
            
        }
        .previewLayout(.fixed(width: 300, height: 50))
    }
}
