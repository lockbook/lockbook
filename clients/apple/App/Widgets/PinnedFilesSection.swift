import SwiftUI
import SwiftWorkspace

struct PinnedFilesSection: View {
    @Environment(FilesModel.self) private var filesModel
    @Environment(HomeState.self) private var homeState
    @Environment(WorkspaceInputState.self) private var workspaceInput

    let fileTreeModel: FileTreeModel
    let showFileTree: () -> Void

    private static let chipHeight: CGFloat = 30
    private static let spacing: CGFloat = 8
    private static let maxRows = 3

    private var pinnedFiles: [File] {
        filesModel.pinnedIds
            .compactMap { filesModel.idsToFiles[$0] }
            .sorted {
                let (a, b) = ($0.name.lowercased(), $1.name.lowercased())

                if a != b {
                    return a < b
                }

                return $0.id < $1.id
            }
    }

    var body: some View {
        let files = pinnedFiles

        if !files.isEmpty {
            VStack(alignment: .leading, spacing: Self.spacing) {
                Text("Pinned")
                    .font(.caption)
                    .foregroundStyle(.secondary)

                if files.count > Self.maxRows * 2 {
                    ScrollView {
                        grid(files)
                    }
                    .frame(height: maxGridHeight)
                } else {
                    grid(files)
                }
            }
            .padding(.horizontal, 10)
            .padding(.bottom, 8)
        }
    }

    private var maxGridHeight: CGFloat {
        CGFloat(Self.maxRows) * Self.chipHeight + CGFloat(Self.maxRows - 1) * Self.spacing
    }

    private func grid(_ files: [File]) -> some View {
        LazyVGrid(
            columns: [
                GridItem(.flexible(), spacing: Self.spacing),
                GridItem(.flexible(), spacing: Self.spacing),
            ],
            spacing: Self.spacing
        ) {
            ForEach(files) { file in
                chip(file)
            }
        }
    }

    private func chip(_ file: File) -> some View {
        HStack(spacing: 5) {
            Image(systemName: icon(for: file))
                .font(.caption)
                .foregroundStyle(Color.accentColor)

            Text(file.name)
                .font(.callout)
                .lineLimit(1)
                .truncationMode(.tail)

            Spacer(minLength: 0)
        }
        .padding(.horizontal, 8)
        .frame(height: Self.chipHeight)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(.quaternary.opacity(0.6), in: RoundedRectangle(cornerRadius: 7))
        .contentShape(RoundedRectangle(cornerRadius: 7))
        .onTapGesture {
            open(file)
        }
        .contextMenu {
            if !file.isFolder {
                contextMenuItem("Open in New Tab", systemImage: "plus.rectangle.on.rectangle") {
                    workspaceInput.openFile(id: file.id, newTab: true)
                    homeState.compactColumn = .detail
                }
            }
            contextMenuItem("Unpin", systemImage: "pin.slash") {
                filesModel.togglePin(file.id)
            }
        }
    }

    private func icon(for file: File) -> String {
        file.isFolder ? "folder.fill" : FileIconHelper.docNameToSystemImageName(name: file.name)
    }

    private func open(_ file: File) {
        if file.isFolder {
            workspaceInput.selectFolder(id: file.id)
            withAnimation {
                fileTreeModel.expandToFile(file)
            }
            showFileTree()
        } else {
            workspaceInput.openFile(id: file.id)
            homeState.compactColumn = .detail
        }
    }
}

#Preview {
    PinnedFilesSection(fileTreeModel: .preview, showFileTree: {})
        .environment(FilesModel.preview)
        .environment(HomeState())
        .environment(WorkspaceInputState.preview)
}
