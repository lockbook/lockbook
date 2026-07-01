import SwiftUI
import SwiftWorkspace

struct FileTreeView: View {
    @Environment(FilesModel.self) private var filesModel
    @EnvironmentObject private var workspaceInput: WorkspaceInputState

    let fileTreeModel: FileTreeModel

    var body: some View {
        if let root = filesModel.root {
            ScrollViewReader { scrollHelper in
                ScrollView {
                    VStack(alignment: .leading, spacing: 2) {
                        FileRowView(file: root, level: -1)
                    }
                    .padding(.horizontal)
                }
                .onChange(of: fileTreeModel.openDoc) {
                    if let openDoc = fileTreeModel.openDoc {
                        scrollHelper.scrollTo(openDoc)
                    }
                }
            }
            .refreshable {
                workspaceInput.requestSync()
            }
            .environment(fileTreeModel)
            .navigationTitle(root.name)
            #if os(iOS)
                .navigationBarTitleDisplayMode(.large)
            #endif
        } else {
            ProgressView()
        }
    }
}

struct FileRowView: View {
    @Environment(FilesModel.self) private var filesModel
    @Environment(FileTreeModel.self) private var fileTreeModel
    @Environment(HomeState.self) private var homeState
    @EnvironmentObject private var workspaceInput: WorkspaceInputState

    let file: File
    let level: CGFloat

    var children: [File] {
        filesModel.childrens[file.id] ?? []
    }

    var isLeaf: Bool {
        children.isEmpty
    }

    var isOpen: Bool {
        fileTreeModel.openFolders.contains(file.id)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            if level != -1 {
                fileRow
                    .onTapGesture {
                        openFile()
                    }
            }

            if !isLeaf, isOpen || level == -1 {
                ForEach(children, id: \.id) { child in
                    FileRowView(file: child, level: level + 1)
                }
            }
        }
        .id(file.id)
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

            if let dot = filesModel.statusDots[file.id] {
                Circle()
                    .fill(dot.color)
                    .frame(width: 8, height: 8)
            }

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
        }
        .padding(.vertical, 9)
        .contentShape(Rectangle())
        .padding(.leading, level * 20 + 5)
        .padding(.trailing, 10)
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
        FileTreeView(
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
