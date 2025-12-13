import SwiftUI
import SwiftWorkspace

struct PendingSharesView: View {
    @EnvironmentObject var homeState: HomeState
    @EnvironmentObject var filesModel: FilesViewModel

    @StateObject var fileTreeModel: FileTreeViewModel

    init(
        filesModel: FilesViewModel,
        workspaceInput: WorkspaceInputState,
        workspaceOutput: WorkspaceOutputState
    ) {
        self._fileTreeModel = StateObject(
            wrappedValue: FileTreeViewModel(
                filesModel: filesModel,
                workspaceInput: workspaceInput,
                workspaceOutput: workspaceOutput
            )
        )
    }

    var body: some View {
        Group {
            if let pendingShares = filesModel.pendingShares {
                ScrollViewReader { scrollHelper in
                    ScrollView {
                        VStack {
                            ForEach(
                                pendingShares.sorted(by: { $0.key < $1.key }),
                                id: \.key
                            ) {
                                username,
                                shares in
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
                                            ForEach(
                                                shares,
                                                content: { file in
                                                    PendingShareRowView(
                                                        file: file,
                                                    )
                                                    .environmentObject(
                                                        fileTreeModel
                                                    )
                                                }
                                            )
                                        }
                                        .padding(.leading)
                                    }
                                )
                            }
                        }
                        .formStyle(.columns)
                    }
                    .onChange(of: fileTreeModel.openDoc) { newValue in
                        scrollHelper.scrollTo(newValue, anchor: .center)
                    }
                }
            } else {
                ProgressView()
            }
        }
        .navigationTitle("Shared with me")
        .toolbarTitleDisplayMode(.large)
    }
}

struct PendingShareRowView: View {
    @EnvironmentObject var homeState: HomeState
    @EnvironmentObject var filesModel: FilesViewModel
    @EnvironmentObject var fileTreeModel: FileTreeViewModel
    @EnvironmentObject var workspaceInput: WorkspaceInputState

    @State var confirmRejection = false

    let file: File
    var level: CGFloat = 1

    var children: [File] {
        let children = (filesModel.childrens[file.id] ?? []).sorted { $1 > $0 }

        return children
    }

    var isRootShare: Bool {
        return level == 1
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
                self.openOrSelectFile()
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

        if !isLeaf && isOpen {
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
                Button(
                    action: {
                        homeState.selectSheetInfo = .acceptShare(
                            name: file.name,
                            id: file.id
                        )
                    },
                    label: {
                        Label("Accept", systemImage: "checkmark.circle.fill")
                            .labelStyle(.iconOnly)
                    }
                )

                Button(
                    action: {
                        self.confirmRejection = true
                    },
                    label: {
                        Label("Reject", systemImage: "xmark.circle.fill")
                            .labelStyle(.iconOnly)
                    }
                )
            }

        }
        .padding(.vertical, 9)
        .contentShape(Rectangle())
        .padding(.leading, level * 20 + 5)
        .padding(.trailing)
        .modifier(OpenDocModifier(file: file))
        .padding(.trailing)
    }

    func openOrSelectFile() {
        homeState.closeWorkspaceBlockingScreens()

        if file.isFolder {
            fileTreeModel.supressNextOpenFolder = true
            workspaceInput.selectFolder(id: file.id)

            withAnimation {
                let _ = fileTreeModel.toggleFolder(file.id)
            }
        } else {
            workspaceInput.openFile(id: file.id)

            if homeState.isSidebarFloating {
                homeState.sidebarState = .closed
            }
        }
    }
}

struct PendingShareFileCell: View {
    @EnvironmentObject var homeState: HomeState
    @EnvironmentObject var filesModel: FilesViewModel
    @Environment(\.dismiss) private var dismiss

    @State var confirmRejection = false

    let file: File

    var body: some View {
        HStack(spacing: 10) {
            Image(systemName: FileIconHelper.fileToSystemImageName(file: file))
                .foregroundColor(
                    file.type == .folder ? Color.accentColor : .secondary
                )
                .imageScale(.large)

            Text(file.name)
                .font(.title3)

            Spacer()

            Button {
                homeState.selectSheetInfo = .acceptShare(
                    name: file.name,
                    id: file.id
                )

                dismiss()
            } label: {
                Image(systemName: "plus.circle")
                    .imageScale(.large)
                    .foregroundColor(Color.accentColor)
            }
            .buttonStyle(.plain)

            Button {
                confirmRejection = true
            } label: {
                Image(systemName: "minus.circle")
                    .imageScale(.large)
                    .foregroundColor(.red)
            }
            .buttonStyle(.plain)
        }
        .padding(.vertical, 7)
        .contentShape(Rectangle())
        .confirmationDialog(
            "Are you sure?",
            isPresented: $confirmRejection,
            titleVisibility: .visible
        ) {
            Button("Reject \"\(file.name)\"", role: .destructive) {
                filesModel.rejectShare(id: file.id)
                
                dismiss()
            }
        }
    }
}

#Preview("Pending Shares") {
    NavigationStack {
        PendingSharesView(
            filesModel: .preview,
            workspaceInput: .preview,
            workspaceOutput: .preview
        )
        .withMacPreviewSize()
        .withCommonPreviewEnvironment()
    }
}
