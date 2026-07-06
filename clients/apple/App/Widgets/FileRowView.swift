import SwiftUI
import SwiftWorkspace

struct FileRowView: View {
    @Environment(FilesModel.self) private var filesModel
    @Environment(FileTreeModel.self) private var fileTreeModel
    @Environment(HomeState.self) private var homeState
    @Environment(WorkspaceInputState.self) private var workspaceInput

    let file: File
    let level: CGFloat

    @State private var isDropTargeted = false
    @State private var springOpenTask: Task<Void, Never>?

    var isLeaf: Bool {
        (filesModel.childrenByParent[file.id] ?? []).isEmpty
    }

    var isOpen: Bool {
        fileTreeModel.openFolders.contains(file.id)
    }

    var body: some View {
        fileRow
            .selectableRow(
                fileTreeModel.selection,
                id: file.id,
                orderedIds: { fileTreeModel.visibleRows.map(\.id) },
                open: openFile
            )
            #if os(iOS)
                .draggable(DraggedFile(file: file, filesModel: filesModel))
            #endif
            .fileDropTarget(isTargeted: { targeted in
                setDropTargeted(targeted)
            }, action: { dropped in
                drop(dropped)
            })
    }

    var fileRow: some View {
        HStack {
            SelectionIndicator(selection: fileTreeModel.selection, id: file.id)

            FileIcon(file: file)

            Text(file.name)
                .lineLimit(1)
                .truncationMode(.tail)
                .allowsTightening(true)
                .foregroundColor(.primary)

            if let dot = filesModel.statusDots[file.id] {
                Circle()
                    .fill(dot.color)
                    .frame(width: 8, height: 8)
            }

            if filesModel.pinnedIds.contains(file.id) {
                Image(systemName: "pin.fill")
                    .font(.system(size: 10))
                    .foregroundStyle(.orange)
            }

            Spacer()

            if !isLeaf {
                DisclosureChevron(isOpen: isOpen)
            }
        }
        .padding(.vertical, 9)
        .contentShape(Rectangle())
        .padding(.leading, level * 20 + 5)
        .padding(.trailing, 10)
        .modifier(OpenDocModifier(file: file))
        .overlay(alignment: .bottom) {
            if isDropTargeted, !file.isFolder {
                Capsule()
                    .fill(Color.accentColor)
                    .frame(height: 2.5)
                    .padding(.leading, level * 20 + 5)
                    .padding(.trailing, 10)
            }
        }
    }

    func openFile() {
        fileTreeModel.open(file, workspaceInput: workspaceInput, homeState: homeState)
    }

    private func setDropTargeted(_ targeted: Bool) {
        withAnimation(.easeInOut(duration: 0.12)) {
            isDropTargeted = targeted
        }

        if targeted {
            fileTreeModel.dropTarget = file
        } else if fileTreeModel.dropTarget == file {
            fileTreeModel.dropTarget = nil
        }

        springOpenTask?.cancel()
        springOpenTask = nil

        if targeted, file.isFolder, !isOpen {
            springOpenTask = Task {
                try? await Task.sleep(for: .milliseconds(650))
                guard !Task.isCancelled else { return }

                withAnimation {
                    _ = fileTreeModel.openFolders.insert(file.id)
                }
            }
        }
    }

    private var dropDestinationFolder: File? {
        file.isFolder ? file : filesModel.idsToFiles[file.parent]
    }

    private func drop(_ dropped: [FileDropItem]) -> Bool {
        guard let dest = dropDestinationFolder, filesModel.drop(dropped, into: dest) else {
            return false
        }

        withAnimation {
            _ = fileTreeModel.openFolders.insert(dest.id)
        }

        return true
    }
}

extension SyncDot {
    var color: Color {
        switch self {
        case .pushing: .green
        case .dirty: .yellow
        case .pulling: .blue
        }
    }
}
