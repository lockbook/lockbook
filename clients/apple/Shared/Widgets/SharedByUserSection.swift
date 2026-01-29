import SwiftUI
import SwiftWorkspace

struct SharedByUserSection: View {
    let username: String
    let shares: [File]

    var body: some View {
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
                        }
                    )
                }
                .padding(.leading)
            }
        )
    }
}

struct PendingShareRowView: View {
    @EnvironmentObject var homeState: HomeState
    @EnvironmentObject var filesModel: FilesViewModel
    @EnvironmentObject var fileTreeModel: FileTreeViewModel
    @EnvironmentObject var workspaceInput: WorkspaceInputState

    @State var confirmRejection = false

    #if os(macOS)
        let isMacOS = true
    #else
        let isMacOS = false
    #endif

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
            if !isLeaf && isMacOS {
                openArrow
            }

            Image(systemName: FileIconHelper.fileToSystemImageName(file: file))
                .font(.system(size: isMacOS ? 13 : 16))
                .frame(width: isMacOS ? 14 : 16)
                .foregroundColor(file.isFolder ? .accentColor : .secondary)

            Text(file.name)
                .lineLimit(1)
                .truncationMode(.tail)
                .allowsTightening(true)
                .foregroundColor(.primary)

            Spacer()

            if !isLeaf && !isMacOS {
                openArrow
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
                .buttonStyle(.borderless)
                .tint(.accentColor)

                Button(
                    role: .destructive,
                    action: {
                        self.confirmRejection = true
                    },
                    label: {
                        Label("Reject", systemImage: "xmark.circle.fill")
                            .labelStyle(.iconOnly)
                    }
                )
                .buttonStyle(.borderless)
                .tint(.red)
            }
        }
        .padding(.vertical, isMacOS ? 5 : 9)
        .contentShape(Rectangle())
        .padding(.leading, level * 20 + 5)
        .padding(.trailing)
        .modifier(OpenDocModifier(file: file))
        .padding(.trailing)
    }

    var openArrow: some View {
        Image(systemName: "chevron.forward")
            .renderingMode(.template)
            .resizable()
            .scaledToFit()
            .frame(width: 10, height: 10)
            .rotationEffect(Angle.degrees(isOpen ? 90 : 0))
            .foregroundColor(isMacOS ? nil : .accentColor)
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
