import SwiftUI
import SwiftWorkspace

struct SharedWithMeView: View {
    @Environment(FilesModel.self) private var filesModel
    @Environment(WorkspaceInputState.self) private var workspaceInput
    @Environment(WorkspaceOutputState.self) private var workspaceOutput
    #if os(iOS)
        @Environment(HomeState.self) private var homeState
    #endif

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
        .largeNavigationTitle()
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
            .refreshable {
                #if os(iOS)
                    homeState.explicitSyncRequested()
                #endif
                workspaceInput.requestSync()
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
    @Environment(\.openWindow) private var openWindow

    @State private var confirmRejection = false
    @State private var showAcceptInto = false

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
            .contextMenu {
                menuItems
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
            .sheet(isPresented: $showAcceptInto) {
                FolderPickerSheet(fileTreeModel: fileTreeModel, selection: nil) { folder in
                    filesModel.acceptShare(file: file, into: folder)
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
            draggableContent

            if isRootShare {
                Button(action: {
                    showAcceptInto = true
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
        .padding(.leading, level * 20 + 5)
        .padding(.trailing)
        .modifier(OpenDocModifier(file: file))
    }

    private var rowContent: some View {
        HStack {
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

            Spacer()

            if !isLeaf {
                DisclosureChevron(isOpen: isOpen)
            }
        }
        .contentShape(Rectangle())
    }

    @ViewBuilder
    private var draggableContent: some View {
        #if os(macOS)
            rowContent.overlay {
                MacFileDragSource(file: file, filesModel: filesModel, onClick: {
                    openFile()
                }, onDoubleClick: {
                    openInNewWindow()
                })
            }
        #else
            rowContent
                .onTapGesture {
                    openFile()
                }
                .simultaneousGesture(
                    TapGesture(count: 2).onEnded {
                        openInNewWindow()
                    }
                )
                .draggable(DraggedFile(file: file, filesModel: filesModel))
        #endif
    }

    @ViewBuilder
    private var menuItems: some View {
        if !file.isFolder, supportsMultipleWindows {
            contextMenuItem("Open in New Window", systemImage: "macwindow.badge.plus") {
                openInNewWindow()
            }

            Divider()
        }

        if isRootShare {
            contextMenuItem("Accept", systemImage: "checkmark.circle") {
                filesModel.acceptShare(file: file)
            }

            contextMenuItem("Accept Into...", systemImage: "folder.badge.plus") {
                showAcceptInto = true
            }
        }

        ShareLink(item: DraggedFile(file: file, filesModel: filesModel), preview: SharePreview(file.name)) {
            Label("Share Externally", systemImage: "square.and.arrow.up")
        }

        contextMenuItem("Copy Link", systemImage: "link") {
            ClipboardHelper.copyFileLink(file.id)
        }

        if isRootShare {
            Divider()

            contextMenuItem("Reject", systemImage: "xmark.circle", role: .destructive) {
                confirmRejection = true
            }
        }
    }

    private var supportsMultipleWindows: Bool {
        #if os(iOS)
            UIApplication.shared.supportsMultipleScenes
        #else
            true
        #endif
    }

    private func openInNewWindow() {
        guard !file.isFolder, supportsMultipleWindows else {
            return
        }

        openWindow(id: documentWindowId, value: file.id)
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
