import SwiftUI
import SwiftWorkspace

struct RecentsView: View {
    @Environment(FilesModel.self) private var filesModel

    let model: RecentsModel
    let fileTreeModel: FileTreeModel

    var recentDocs: [File] {
        filesModel.idsToFiles.values
            .filter { $0.type == .document }
            .sorted {
                if $0.lastModified != $1.lastModified {
                    return $0.lastModified > $1.lastModified
                }

                return $0.id.uuidString < $1.id.uuidString
            }
    }

    var body: some View {
        let docs = recentDocs

        Group {
            if docs.isEmpty {
                noDocs
            } else {
                ScrollView {
                    LazyVStack(alignment: .leading, spacing: 0) {
                        ForEach(docs) { file in
                            VStack(spacing: 0) {
                                RecentDocRow(
                                    file: file,
                                    model: model,
                                    fileTreeModel: fileTreeModel,
                                    orderedIds: docs.map(\.id)
                                )
                                Divider()
                            }
                        }
                    }
                }
            }
        }
        .fileDropTarget { items in
            guard let root = filesModel.root else {
                return false
            }

            return filesModel.drop(items, into: root)
        }
        .selectionCommands(model.selection, fileTreeModel: fileTreeModel)
        .navigationTitle("Recents")
        #if os(iOS)
            .navigationBarTitleDisplayMode(.large)
        #endif
    }

    var noDocs: some View {
        VStack(spacing: 6) {
            Text("No documents yet")
                .font(.title3)
                .fontWeight(.semibold)

            Text("Documents you edit will appear here.")
                .font(.body)
                .foregroundStyle(.secondary)
        }
        .multilineTextAlignment(.center)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

struct RecentDocRow: View {
    @Environment(FilesModel.self) private var filesModel
    @Environment(HomeState.self) private var homeState
    @Environment(WorkspaceInputState.self) private var workspaceInput

    let file: File
    let model: RecentsModel
    let fileTreeModel: FileTreeModel
    let orderedIds: [UUID]

    var body: some View {
        HStack(alignment: .top, spacing: 12) {
            SelectionIndicator(selection: model.selection, id: file.id)
                .padding(.top, 1)

            Image(systemName: FileIconHelper.docNameToSystemImageName(name: file.name))
                .font(.title3)
                .foregroundColor(.accentColor)
                .frame(width: 22)
                .padding(.top, 1)

            VStack(alignment: .leading, spacing: 4) {
                HStack(alignment: .firstTextBaseline) {
                    Text(file.name)
                        .fontWeight(.medium)
                        .lineLimit(1)
                        .truncationMode(.tail)

                    if filesModel.pinnedIds.contains(file.id) {
                        Image(systemName: "pin.fill")
                            .font(.system(size: 10))
                            .foregroundStyle(.orange)
                    }

                    Spacer(minLength: 8)

                    Text(min(modifiedDate, .now), format: .relative(presentation: .named))
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }

                FileBreadcrumb(file: file)

                if let preview = model.preview(for: file), !preview.characters.isEmpty {
                    Text(preview)
                        .font(.subheadline)
                        .foregroundStyle(.secondary)
                        .lineLimit(2)
                }

                if let username = model.username, file.lastModifiedBy != username {
                    Label(file.lastModifiedBy, systemImage: "person.fill")
                        .font(.caption.weight(.medium))
                        .foregroundStyle(Color.accentColor)
                        .padding(.horizontal, 8)
                        .padding(.vertical, 3)
                        .background(Capsule().fill(Color.accentColor.opacity(0.15)))
                        .padding(.top, 2)
                }
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(.horizontal, 12)
        .padding(.vertical, 10)
        .contentShape(Rectangle())
        .selectableRow(
            model.selection,
            id: file.id,
            fileTreeModel: fileTreeModel,
            orderedIds: { orderedIds },
            open: {
                workspaceInput.openFile(id: file.id)
                homeState.compactColumn = .detail
            }
        )
        .onAppear {
            model.loadPreviewIfNeeded(file)
        }
    }

    private var modifiedDate: Date {
        Date(timeIntervalSince1970: TimeInterval(file.lastModified) / 1000)
    }

}

#Preview {
    NavigationStack {
        RecentsView(model: RecentsModel(), fileTreeModel: .preview)
    }
    .environment(FilesModel.preview)
    .environment(HomeState())
    .environment(WorkspaceInputState.preview)
}
