import SwiftUI
import SwiftLockbookCore
import SwiftWorkspace

struct OutlineRow: View {
    
    @EnvironmentObject var files: FileService
    @EnvironmentObject var workspace: WorkspaceState
    
    var file: File
    var level: CGFloat
    @Binding var open: Bool
    
    var children: [File] {
        files.files.filter {
            $0.parent == file.id && $0.id != file.id
        }
    }
    
    var isLeaf: Bool {
        children.isEmpty
    }
    
    var body: some View {
        HStack {
            Image(systemName: FileService.metaToSystemImage(meta: file))
                .resizable()
                .scaledToFit()
                .frame(width: 16, height: 16)
                .foregroundColor(file.fileType == .Folder ? .accentColor : (workspace.openDoc == file.id ? .white : .secondary ))
            
            Text(file.name)
                .lineLimit(1) // If lineLimit is not specified, non-leaf names will wrap
                .truncationMode(.tail)
                .allowsTightening(true)
            
            Spacer()
            if !isLeaf {
                Image(systemName: "chevron.forward")
                    .renderingMode(.template)
                    .resizable()
                    .scaledToFit()
                    .frame(width: 10, height: 10)
                    .rotationEffect(Angle.degrees(open ? 90 : 0))
                    .foregroundColor(.accentColor)
            }
        }
        .padding(.vertical, 4)
        .contentShape(Rectangle())
        .padding(.leading, level * 20 + 5)
        .padding(.trailing, 10)
    }
}
