import SwiftUI
import SwiftLockbookCore
import SwiftWorkspace

struct OutlineRow: View {
    
    @EnvironmentObject var files: FileService
    @EnvironmentObject var workspace: WorkspaceState
    
    var file: File
    var level: CGFloat
    @Binding var open: Bool
    let isParentSelected: Bool
    
    var children: [File] {
        files.files.filter {
            $0.parent == file.id && $0.id != file.id
        }
    }
    
    var isLeaf: Bool {
        children.isEmpty
    }
    
    var isSelected: Bool {
        files.selectedFiles?.contains(where: { $0.id == file.id }) == true
    }
    
    var isSelectable: Bool {
        files.selectedFiles != nil
    }
    
    var body: some View {
        HStack {
            if isSelectable {
                ZStack {
                    if isSelected || isParentSelected {
                        Image(systemName: "circle.fill")
                            .foregroundStyle(.blue)
                            .font(.system(size: 17))
                    }
                    
                    Image(systemName: isSelected ? "checkmark" : "circle")
                        .foregroundStyle(isSelected ? Color.white : Color.secondary)
                        .font(.system(size: (isSelected ? 10 : 17)))
                }
                
            }
            
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
        .padding(.vertical, 9)
        .contentShape(Rectangle())
        .padding(.leading, level * 20 + 5)
        .padding(.trailing, 10)
        .modifier(SelectedBranchViewModifier(id: file.id, openDoc: workspace.openDoc, selectedFiles: files.selectedFiles))
    }
}

struct SelectedBranchViewModifier: ViewModifier {
    let isOpenDoc: Bool
    let isSelected: Bool
    
    init(id: UUID, openDoc: UUID?, selectedFiles: [File]?) {
        self.isOpenDoc = id == openDoc
        self.isSelected = selectedFiles?.contains(where: { $0.id == id }) == true
    }
    
    func body(content: Content) -> some View {
        if isSelected {
            content
                .background(isSelected ? .gray.opacity(0.2) : .clear)
        } else if isOpenDoc {
            content
                .foregroundColor(Color.white)
                .background(RoundedRectangle(cornerRadius: 5, style: .continuous).foregroundStyle(Color.accentColor))
        } else {
            content
        }
    }
}

