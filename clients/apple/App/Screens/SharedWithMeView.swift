import SwiftUI
import SwiftWorkspace

struct SharedWithMeView: View {
    @Environment(FilesModel.self) private var filesModel
    @Environment(WorkspaceOutputState.self) private var workspaceOutput

    let fileTreeModel: FileTreeModel

    var body: some View {
        Group {
            if let pendingShares = filesModel.pendingSharesByUsername {
                if pendingShares.isEmpty {
                    EmptyStateView(
                        title: "Nothing shared yet",
                        subtitle: "Files shared with you will appear here."
                    )
                } else {
                    sharedByUsers(pendingShares: pendingShares)
                }
            } else {
                ProgressView()
            }
        }
        .navigationTitle("Shared with me")
        #if os(iOS)
            .navigationBarTitleDisplayMode(.large)
        #endif
    }

    func sharedByUsers(pendingShares: [String: [File]]) -> some View {
        ScrollViewReader { scrollHelper in
            ScrollView {
                VStack {
                    ForEach(pendingShares.sorted(by: { $0.key < $1.key }), id: \.key) { username, shares in
                        CollapsableSection(
                            id: "Shared_\(username)",
                            label: {
                                Text(username)
                                    .bold()
                                    .foregroundColor(.primary)
                                    .textCase(.none)
                                    .font(.headline)
                                    .padding(.bottom, 3)
                                    .padding(.top, 8)
                            },
                            content: {
                                VStack(spacing: 0) {
                                    ForEach(shares) { file in
                                        PendingShareRowView(file: file)
                                    }
                                }
                                .padding(.leading)
                            }
                        )
                    }
                }
            }
            .onChange(of: workspaceOutput.openDoc) {
                if let openDoc = workspaceOutput.openDoc {
                    scrollHelper.scrollTo(openDoc, anchor: .center)
                }
            }
        }
        .environment(fileTreeModel)
    }

}

struct PendingShareRowView: View {
    @Environment(FilesModel.self) private var filesModel
    @Environment(FileTreeModel.self) private var fileTreeModel
    @Environment(HomeState.self) private var homeState
    @Environment(WorkspaceInputState.self) private var workspaceInput

    @State private var confirmRejection = false

    let file: File
    var level: CGFloat = 1

    var children: [File] {
        filesModel.childrenByParent[file.id] ?? []
    }

    var isRootShare: Bool {
        level == 1
    }

    var isLeaf: Bool {
        children.isEmpty
    }

    var isOpen: Bool {
        fileTreeModel.openFolders.contains(file.id)
    }

    var body: some View {
        fileRow
            .onTapGesture {
                openFile()
            }
            .confirmationDialog(
                "Are you sure?",
                isPresented: $confirmRejection,
                titleVisibility: .visible
            ) {
                Button("Reject \"\(file.name)\"", role: .destructive) {
                    filesModel.rejectShare(id: file.id)
                }
            }
            .id(file.id)

        if !isLeaf, isOpen {
            ForEach(children, id: \.id) { child in
                PendingShareRowView(file: child, level: level + 1)
            }
        }
    }

    var fileRow: some View {
        HStack {
            FileIcon(file: file)

            Text(file.name)
                .lineLimit(1)
                .truncationMode(.tail)
                .allowsTightening(true)
                .foregroundColor(.primary)

            Spacer()

            if !isLeaf {
                DisclosureChevron(isOpen: isOpen)
            }

            if isRootShare {
                Button(action: {
                    filesModel.acceptShare(file: file)
                }, label: {
                    Label("Accept", systemImage: "checkmark.circle.fill")
                        .labelStyle(.iconOnly)
                })
                .buttonStyle(.borderless)
                .tint(.accentColor)

                Button(role: .destructive, action: {
                    confirmRejection = true
                }, label: {
                    Label("Reject", systemImage: "xmark.circle.fill")
                        .labelStyle(.iconOnly)
                })
                .buttonStyle(.borderless)
                .tint(.red)
            }
        }
        .padding(.vertical, 9)
        .contentShape(Rectangle())
        .padding(.leading, level * 20 + 5)
        .padding(.trailing)
        .modifier(OpenDocModifier(file: file))
    }

    func openFile() {
        fileTreeModel.open(file, workspaceInput: workspaceInput, homeState: homeState)
    }
}

#Preview {
    let filesModel = FilesModel.preview

    NavigationStack {
        SharedWithMeView(fileTreeModel: FileTreeModel(filesModel: filesModel))
    }
    .environment(filesModel)
    .environment(HomeState())
    .environment(WorkspaceInputState.preview)
    .environment(WorkspaceOutputState.preview)
}
