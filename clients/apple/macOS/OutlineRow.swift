import SwiftUI
import SwiftLockbookCore

/// This view handles displaying the contents of each row for its object. Clicking its arrow image also toggles a node's open state./
struct OutlineRow: View {
    @ObservedObject var core: GlobalState
    var file: FileMetadata
    var level: CGFloat
    @Binding var open: Bool
    @Binding var dragging: FileMetadata?

    var children: [FileMetadata] {
        core.files.filter {
            $0.parent == file.id && $0.id != file.id
        }
    }

    var isLeaf: Bool {
        children.isEmpty
    }

    var body: some View {
        HStack {
            Group {
                if !isLeaf {
                    Image(systemName: open == false ? "arrowtriangle.right.fill" : "arrowtriangle.down.fill")
                        .renderingMode(.template)
                        .foregroundColor(Color.secondary)
                } else {
                    Image(systemName: "arrowtriangle.right.fill")
                        .opacity(0)
                }
            }
            .frame(width: 16, height: 16)
            .onTapGesture {
                withAnimation {
                    open.toggle()
                }
            }

            Image(systemName: file.fileType == .Folder ? "folder" : "doc")
                .renderingMode(.template)
                .frame(width: 16, height: 16)
                .padding(.leading, -4)

            Text(file.name)
                .lineLimit(1) // If lineLimit is not specified, non-leaf names will wrap
                .truncationMode(.tail)
                .allowsTightening(true)

            Spacer()
        }
        .padding(.vertical, 4)
        .contentShape(Rectangle())
        .padding(.leading, level * 20)
        .onDrag {
            self.dragging = file
            return NSItemProvider(object: self.file.id.uuidString as NSString)
        }
    }
}

struct SyntheticOutlineRow: View {
    let fileType: FileType
    var level: CGFloat
    let onCreate: (String) -> Void
    let onCancel: () -> Void

    @State var nameField: String = ""

    var body: some View {
        HStack {
            Group {
                Image(systemName: "plus")
            }
            .frame(width: 16, height: 16)
            Image(systemName: fileType == .Folder ? "folder" : "doc")
                .renderingMode(.template)
                .frame(width: 16, height: 16)
                .padding(.leading, -4)

            TextField("\(fileType.rawValue.lowercased()) name", text: $nameField, onCommit: {
                if (!nameField.isEmpty) {
                    onCreate(nameField)
                } else {
                    onCancel()
                }
            })

            Spacer()
        }
        .padding(.vertical, 4)
        .contentShape(Rectangle())
        .padding(.leading, level * 20)
    }
}
