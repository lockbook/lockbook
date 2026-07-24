import SwiftUI
import SwiftWorkspace

struct CreateFileSheet: View {
    @Environment(\.dismiss) private var dismiss
    @Environment(FilesModel.self) private var filesModel
    @Environment(HomeState.self) private var homeState
    @Environment(WorkspaceInputState.self) private var workspaceInput
    @Environment(WorkspaceOutputState.self) private var workspaceOutput

    enum Mode {
        case create(location: LocationChoice)
        case rename(File)
    }

    let fileTreeModel: FileTreeModel
    let renameTarget: File?

    init(fileTreeModel: FileTreeModel, mode: Mode = .create(location: .root)) {
        self.fileTreeModel = fileTreeModel

        switch mode {
        case let .create(location):
            renameTarget = nil
            _location = State(initialValue: location)
        case let .rename(file):
            renameTarget = file

            let (base, ext) = Self.splitName(file)
            _name = State(initialValue: base)
            _renameExt = State(initialValue: ext)
        }
    }

    @State private var type: NewFileType = .markdown
    @State private var renameExt: String? = nil
    @State private var name = Self.defaultName
    @State private var lastAutoName = Self.defaultName
    @State private var selection: TextSelection? = nil
    @State private var location: LocationChoice = .root
    @State private var showFolderPicker = false
    @State private var error = ""
    @FocusState private var nameFocused: Bool

    static var defaultName: String {
        Date.now.formatted(
            Date.ISO8601FormatStyle(timeZone: .current).year().month().day().dateSeparator(.dash)
        )
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            #if os(macOS)
                HStack {
                    Text(renameTarget == nil ? "New File" : "Rename")
                        .font(.headline)

                    Spacer()

                    SheetCloseButton()
                }
            #endif

            if renameTarget == nil {
                Picker("Type", selection: $type) {
                    ForEach(NewFileType.allCases) { type in
                        Text(type.label).tag(type)
                    }
                }
                .pickerStyle(.segmented)
                .labelsHidden()
            }

            HStack(spacing: 2) {
                TextField("Name", text: $name, selection: $selection)
                    .textFieldStyle(.plain)
                    .autocapitalizationDisabled()
                    .focused($nameFocused)
                    .onSubmit(submit)

                if let ext = displayExt {
                    Text(ext)
                        .foregroundStyle(.secondary)
                }

                #if os(iOS)
                    if nameFocused, !name.isEmpty {
                        Button {
                            selection = nil
                            name = ""
                        } label: {
                            Image(systemName: "xmark.circle.fill")
                                .foregroundStyle(.tertiary)
                        }
                        .buttonStyle(.plain)
                        .padding(.leading, 4)
                    }
                #endif
            }
            .padding(10)
            .background(RoundedRectangle(cornerRadius: 8).fill(Color.gray.opacity(0.15)))

            locationRow

            Text(error)
                .foregroundStyle(.red)
                .fontWeight(.bold)
                .lineLimit(1, reservesSpace: true)

            Spacer(minLength: 0)

            Button(action: submit) {
                Text(renameTarget == nil ? "Create" : "Save")
                    .frame(maxWidth: .infinity)
            }
            .buttonStyle(.borderedProminent)
            .disabled(name.isEmpty)
        }
        .padding()
        .onAppear {
            if let renameTarget, let parent = filesModel.idsToFiles[renameTarget.parent] {
                location = parent.isRoot ? .root : .custom(parent)
            }

            refreshAutoName()

            #if os(macOS)
                nameFocused = true
                selection = TextSelection(range: name.startIndex ..< name.endIndex)
            #endif
        }
        .onChange(of: location) {
            refreshAutoName()
        }
        .onChange(of: type) {
            refreshAutoName()
        }
        #if os(iOS)
            .onChange(of: nameFocused) { _, focused in
                if focused {
                    selection = TextSelection(range: name.startIndex ..< name.endIndex)
                }
            }
        #endif
        .sheet(isPresented: $showFolderPicker) {
            FolderPickerSheet(fileTreeModel: fileTreeModel, selection: customFolder) { folder in
                location = .custom(folder)
            }
        }
        #if os(iOS)
            .presentationDetents([.height(330)])
        #else
            .frame(width: 420, height: 300)
        #endif
    }

    private var locationRow: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("Location")
                .font(.callout)
                .foregroundStyle(.secondary)

            ScrollView(.horizontal, showsIndicators: false) {
                HStack(spacing: 8) {
                    locationChip("Home", systemImage: "house.fill", isSelected: location == .root) {
                        location = .root
                    }

                    if let doc = openDocFile, alongsideFolder != nil, doc.id != renameTarget?.id {
                        locationChip(
                            "Alongside \(doc.name)",
                            systemImage: "arrow.turn.down.right",
                            isSelected: location == .alongside
                        ) {
                            location = .alongside
                        }
                    }

                    locationChip(
                        customChipLabel,
                        systemImage: "folder.fill",
                        isSelected: customFolder != nil
                    ) {
                        showFolderPicker = true
                    }
                }
            }
        }
    }

    private func locationChip(
        _ title: String, systemImage: String, isSelected: Bool, action: @escaping () -> Void
    ) -> some View {
        Button(action: action) {
            Label(title, systemImage: systemImage)
                .font(.callout)
                .lineLimit(1)
                .padding(.horizontal, 10)
                .padding(.vertical, 6)
                .foregroundStyle(isSelected ? Color.accentColor : .primary)
                .background(
                    Capsule().fill(
                        isSelected ? Color.accentColor.opacity(0.2) : Color.gray.opacity(0.15)
                    )
                )
                .contentShape(Capsule())
        }
        .buttonStyle(.plain)
    }

    private var openDocFile: File? {
        workspaceOutput.openDoc.flatMap { filesModel.idsToFiles[$0] }
    }

    private var alongsideFolder: File? {
        openDocFile.flatMap { filesModel.idsToFiles[$0.parent] }
    }

    private var customFolder: File? {
        guard case let .custom(folder) = location else {
            return nil
        }

        return folder
    }

    private var customChipLabel: String {
        guard let customFolder else {
            return "Choose..."
        }

        return customFolder.isRoot ? "Home" : customFolder.name
    }

    private var destination: File? {
        switch location {
        case .root: filesModel.root
        case .alongside: alongsideFolder ?? filesModel.root
        case let .custom(folder): folder
        }
    }

    private var displayExt: String? {
        renameTarget == nil ? type.ext : renameExt
    }

    private static func splitName(_ file: File) -> (String, String?) {
        guard !file.isFolder,
              let dot = file.name.lastIndex(of: "."),
              dot != file.name.startIndex
        else {
            return (file.name, nil)
        }

        return (String(file.name[..<dot]), String(file.name[dot...]))
    }

    private func refreshAutoName() {
        guard renameTarget == nil, name == lastAutoName, let parent = destination else {
            return
        }

        let desired = Self.defaultName + (type.ext ?? "")

        guard case let .success(next) = AppState.lb.nextName(parent: parent.id, desired: desired) else {
            return
        }

        var base = next
        if let ext = type.ext, base.hasSuffix(ext) {
            base = String(base.dropLast(ext.count))
        }

        guard base != name else {
            return
        }

        selection = nil
        name = base
        lastAutoName = base

        if nameFocused {
            selection = TextSelection(range: name.startIndex ..< name.endIndex)
        }
    }

    private func submit() {
        if let renameTarget {
            save(renameTarget)
        } else {
            create()
        }
    }

    private func save(_ file: File) {
        guard !name.isEmpty, let dest = destination else {
            return
        }

        let fullName = name + (renameExt ?? "")

        if fullName != file.name {
            if case let .failure(err) = AppState.lb.renameFile(id: file.id, newName: fullName) {
                error = err.msg
                return
            }

            workspaceInput.fileOpCompleted(fileOp: .Rename(id: file.id, newName: fullName))
        }

        if dest.id != file.parent {
            guard filesModel.moveFiles([file], into: dest) else {
                error = "Cannot move \"\(fullName)\" into \"\(dest.isRoot ? "Home" : dest.name)\""
                filesModel.loadFiles()
                return
            }
        } else {
            filesModel.loadFiles()
        }

        dismiss()
    }

    private func create() {
        guard !name.isEmpty, let parent = destination else {
            return
        }

        let fullName = name + (type.ext ?? "")
        let fileType: FileType = type == .folder ? .folder : .document

        switch AppState.lb.createFile(name: fullName, parent: parent.id, fileType: fileType) {
        case let .success(file):
            filesModel.loadFiles()

            if !file.isFolder {
                workspaceInput.openFile(id: file.id, newTab: true)
                homeState.compactColumn = .detail
            }

            dismiss()
        case let .failure(err):
            error = err.msg
        }
    }
}

enum LocationChoice: Equatable {
    case root
    case alongside
    case custom(File)

    init(folder: File) {
        self = folder.isRoot ? .root : .custom(folder)
    }
}

enum NewFileType: CaseIterable, Identifiable {
    case markdown
    case drawing
    case folder
    case other

    var id: Self {
        self
    }

    var label: String {
        switch self {
        case .markdown: "Note"
        case .drawing: "Drawing"
        case .folder: "Folder"
        case .other: "Other"
        }
    }

    var ext: String? {
        switch self {
        case .markdown: ".md"
        case .drawing: ".svg"
        case .folder: nil
        case .other: nil
        }
    }
}

#Preview {
    Color.clear
        .sheet(isPresented: .constant(true)) {
            CreateFileSheet(fileTreeModel: .preview)
        }
        .environment(FilesModel.preview)
        .environment(HomeState())
        .environment(WorkspaceInputState.preview)
        .environment(WorkspaceOutputState.preview)
}
