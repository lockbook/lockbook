import SwiftUI
import SwiftWorkspace

struct OutlineRow: View {
    
    @EnvironmentObject var files: FileService
    @EnvironmentObject var selected: SelectedFilesState
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
    
    var isSelected: Bool {
        selected.totalSelectedFiles?.contains(file) == true
    }
    
    var isSelectable: Bool {
        selected.selectedFiles != nil
    }
    
    var body: some View {
        HStack {
            if isSelectable {
                ZStack {
                    if isSelected {
                        Image(systemName: "circle.fill")
                            .foregroundStyle(.blue)
                            .font(.system(size: 17))
                    }
                    
                    Image(systemName: isSelected ? "checkmark" : "circle")
                        .foregroundStyle(isSelected ? Color.white : Color.secondary)
                        .font(.system(size: (isSelected ? 10 : 17)))
                }
                .padding(.trailing, 5)
                
            }
            
            Image(systemName: FileService.metaToSystemImage(meta: file))
                .font(.system(size: 16))
                .frame(width: 16)
                .foregroundColor(file.type == .folder ? .accentColor : (workspace.openDoc == file.id && !isSelected ? .white : .secondary ))
            
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
        .modifier(SelectedBranchViewModifier(file: file, openDoc: workspace.openDoc, selectedFiles: selected.totalSelectedFiles))
    }
}

struct SelectedBranchViewModifier: ViewModifier {
    let isOpenDoc: Bool
    let isSelected: Bool
    
    init(file: File, openDoc: UUID?, selectedFiles: Set<File>?) {
        self.isOpenDoc = file.id == openDoc
        self.isSelected = selectedFiles?.contains(file) == true
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

