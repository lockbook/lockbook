import SwiftUI
import SwiftLockbookCore

struct FileCell: View {
    let meta: FileMetadata
    
    var body: some View {
        HStack {
            Image(systemName: meta.fileType == .Folder ? "folder" : "doc")
                .frame(width: 40, height: 40, alignment: .center)
                .foregroundColor( meta.fileType == .Folder ? .blue : .white)
            Text(meta.name)
            Spacer()
        }.background(Color(UIColor.tertiarySystemFill))
    }
}

struct FileCell_Previews: PreviewProvider {
    static var previews: some View {
        Group {
            FileCell(meta: Core().files[0])
        }
        .previewLayout(.fixed(width: 300, height: 50))
    }
}
