import SwiftUI
import SwiftLockbookCore

struct FileCell: View {
    let meta: FileMetadata

    var body: some View {
        VStack(alignment: .leading) {
            Text(meta.name)
            Label(intEpochToString(epoch: meta.contentVersion), systemImage: meta.fileType == .Folder ? "folder" : "doc")
                .font(.footnote)
                .foregroundColor(.secondary)
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
                TextField("new file...", text: $nameField)
                    .autocapitalization(.none)
                Label("New \(params.1.rawValue) in \(params.0.name)", systemImage: params.1 == .Folder ? "folder" : "doc")
                    .font(.footnote)
                    .foregroundColor(.secondary)
            }
            Button(action: onCreate) {
                Image(systemName: "plus")
                    .foregroundColor(.green)
            }
            .padding(.horizontal, 10)
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
