import SwiftUI
import SwiftWorkspace

struct FolderPickerSheet: View {
    @Environment(\.dismiss) private var dismiss
    @Environment(FilesModel.self) private var filesModel

    let fileTreeModel: FileTreeModel
    let onPick: (File) -> Void

    @State private var selected: File?
    @State private var query = ""

    init(fileTreeModel: FileTreeModel, selection: File?, onPick: @escaping (File) -> Void) {
        self.fileTreeModel = fileTreeModel
        self.onPick = onPick
        _selected = State(initialValue: selection)
    }

    private var selectedFolder: File? {
        selected ?? filesModel.root
    }

    private var searchResults: [File] {
        Array(
            filesModel.idsToFiles.values
                .filter {
                    $0.isFolder && !$0.isRoot
                        && $0.name.localizedCaseInsensitiveContains(query)
                }
                .sorted()
                .prefix(50)
        )
    }

    var body: some View {
        VStack(spacing: 0) {
            header
            SearchField(placeholder: "Search folders", text: $query)
            Divider()
            list
            Divider()
            footer
        }
        #if os(macOS)
            .frame(width: 420, height: 480)
        #endif
    }

    private var header: some View {
        HStack {
            Text("Choose Location")
                .font(.headline)

            Spacer()

            #if os(macOS)
                SheetCloseButton()
            #else
                Button("Cancel") {
                    dismiss()
                }
                .keyboardShortcut(.cancelAction)
            #endif
        }
        .padding(.horizontal)
        .padding(.top, 14)
        .padding(.bottom, 4)
    }

    private var list: some View {
        ScrollView {
            LazyVStack(spacing: 2) {
                if query.isEmpty {
                    if let root = filesModel.root {
                        FolderPickerRow(
                            file: root,
                            level: 0,
                            isOpen: true,
                            isSelected: selectedFolder?.id == root.id,
                            showsPath: false
                        ) {
                            selected = root
                        }
                    }

                    ForEach(fileTreeModel.visibleRows) { row in
                        FolderPickerRow(
                            file: row.file,
                            level: row.level + 1,
                            isOpen: fileTreeModel.openFolders.contains(row.id),
                            isSelected: selectedFolder?.id == row.id,
                            showsPath: false
                        ) {
                            selected = row.file

                            withAnimation {
                                fileTreeModel.toggleFolder(row.id)
                            }
                        }
                    }
                } else {
                    ForEach(searchResults) { file in
                        FolderPickerRow(
                            file: file,
                            level: 0,
                            isOpen: false,
                            isSelected: selectedFolder?.id == file.id,
                            showsPath: true
                        ) {
                            selected = file
                            fileTreeModel.expandToFile(file)
                            query = ""
                        }
                    }
                }
            }
            .id(query.isEmpty)
            .padding(.horizontal)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private var footer: some View {
        Button(action: confirm) {
            Text("Select \"\(selectedFolder.map { $0.isRoot ? "Home" : $0.name } ?? "")\"")
                .lineLimit(1)
                .frame(maxWidth: .infinity)
        }
        .buttonStyle(.borderedProminent)
        .disabled(selectedFolder == nil)
        .padding()
    }

    private func confirm() {
        guard let folder = selectedFolder else {
            return
        }

        onPick(folder)
        dismiss()
    }
}

struct FolderPickerRow: View {
    @Environment(FilesModel.self) private var filesModel

    let file: File
    let level: CGFloat
    let isOpen: Bool
    let isSelected: Bool
    let showsPath: Bool
    let onTap: () -> Void

    var isLeaf: Bool {
        (filesModel.childrenByParent[file.id] ?? []).isEmpty
    }

    var body: some View {
        HStack {
            FileIcon(file: file, systemName: file.isRoot ? "house.fill" : nil)

            VStack(alignment: .leading, spacing: 2) {
                Text(file.isRoot ? "Home" : file.name)
                    .lineLimit(1)
                    .truncationMode(.tail)
                    .allowsTightening(true)
                    .foregroundColor(.primary)

                if showsPath {
                    FileBreadcrumb(file: file, includeHome: true)
                }
            }

            Spacer()

            if !showsPath, file.isFolder, !isLeaf, !file.isRoot {
                DisclosureChevron(isOpen: isOpen)
            }
        }
        .padding(.vertical, 9)
        .padding(.leading, level * 20 + 5)
        .padding(.trailing, 10)
        .background(
            RoundedRectangle(cornerRadius: 6)
                .fill(isSelected ? Color.accentColor.opacity(0.15) : Color.clear)
        )
        .contentShape(Rectangle())
        .opacity(file.isFolder ? 1 : 0.4)
        .onTapGesture {
            if file.isFolder {
                onTap()
            }
        }
    }
}

#Preview {
    Color.clear
        .sheet(isPresented: .constant(true)) {
            FolderPickerSheet(fileTreeModel: .preview, selection: nil) { _ in }
        }
        .environment(FilesModel.preview)
}
