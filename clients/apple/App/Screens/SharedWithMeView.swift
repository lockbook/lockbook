import SwiftUI
import SwiftWorkspace

struct SharedWithMeView: View {
    @Environment(FilesModel.self) private var filesModel

    let fileTreeModel: FileTreeModel

    var body: some View {
        Group {
            if let pendingShares = filesModel.pendingSharesByUsername {
                if pendingShares.isEmpty {
                    noShares
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
            .onChange(of: fileTreeModel.openDoc) {
                if let openDoc = fileTreeModel.openDoc {
                    scrollHelper.scrollTo(openDoc, anchor: .center)
                }
            }
        }
        .environment(fileTreeModel)
    }

    var noShares: some View {
        VStack(spacing: 6) {
            Text("Nothing shared yet")
                .font(.title3)
                .fontWeight(.semibold)

            Text("Files shared with you will appear here.")
                .font(.body)
                .foregroundStyle(.secondary)
        }
        .multilineTextAlignment(.center)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

struct PendingShareRowView: View {
    @Environment(FilesModel.self) private var filesModel
    @Environment(FileTreeModel.self) private var fileTreeModel
    @Environment(HomeState.self) private var homeState
    @EnvironmentObject private var workspaceInput: WorkspaceInputState

    @State private var confirmRejection = false

    let file: File
    var level: CGFloat = 1

    var children: [File] {
        filesModel.childrens[file.id] ?? []
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
            Image(systemName: FileIconHelper.fileToSystemImageName(file: file))
                .font(.system(size: 16))
                .frame(width: 16)
                .foregroundColor(file.isFolder ? .accentColor : .secondary)

            Text(file.name)
                .lineLimit(1)
                .truncationMode(.tail)
                .allowsTightening(true)
                .foregroundColor(.primary)

            Spacer()

            if !isLeaf {
                Image(systemName: "chevron.forward")
                    .renderingMode(.template)
                    .resizable()
                    .scaledToFit()
                    .frame(width: 10, height: 10)
                    .rotationEffect(Angle.degrees(isOpen ? 90 : 0))
                    .foregroundColor(.accentColor)
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
        if file.isFolder {
            fileTreeModel.suppressNextFolderSelection = true
            workspaceInput.selectFolder(id: file.id)

            withAnimation {
                fileTreeModel.toggleFolder(file.id)
            }
        } else {
            workspaceInput.openFile(id: file.id)
            homeState.compactColumn = .detail
        }
    }
}

#Preview {
    let filesModel = FilesModel.preview

    NavigationStack {
        SharedWithMeView(
            fileTreeModel: FileTreeModel(
                filesModel: filesModel,
                workspaceOutput: .preview
            )
        )
    }
    .environment(filesModel)
    .environment(HomeState())
    .environmentObject(WorkspaceInputState.preview)
}
