import SwiftUI
import SwiftWorkspace

private let swipeThreshold: CGFloat = 60

private func rubberBand(_ value: CGFloat, threshold: CGFloat) -> CGFloat {
    value < threshold ? value : threshold + (value - threshold) * 0.2
}

private func selectRevealIndicator(offset: CGFloat) -> some View {
    Image(systemName: "checkmark.circle")
        .font(.system(size: 20))
        .foregroundStyle(Color.accentColor)
        .scaleEffect(min(offset / swipeThreshold, 1))
        .opacity(min(offset / swipeThreshold, 1))
        .frame(width: offset)
}

func contextMenuItem(
    _ title: String,
    systemImage: String,
    role: ButtonRole? = nil,
    action: @escaping () -> Void
) -> some View {
    Button(role: role, action: action) {
        #if os(macOS)
            Label(title, systemImage: systemImage)
                .frame(minWidth: 180, alignment: .leading)
        #else
            Label(title, systemImage: systemImage)
        #endif
    }
}

struct SelectionIndicator: View {
    let selection: SelectionModel
    let id: UUID

    var body: some View {
        if selection.selecting {
            let isSelected = selection.selectedIds.contains(id)

            Image(systemName: isSelected ? "checkmark.circle.fill" : "circle")
                .font(.system(size: 16))
                .foregroundStyle(isSelected ? Color.accentColor : Color.secondary)
        }
    }
}

struct SelectableRowModifier: ViewModifier {
    @Environment(FilesModel.self) private var filesModel
    @Environment(FileTreeModel.self) private var fileTreeModel
    @Environment(WorkspaceInputState.self) private var workspaceInput
    @Environment(\.openWindow) private var openWindow

    let selection: SelectionModel
    let id: UUID
    let orderedIds: () -> [UUID]
    let open: () -> Void

    @State private var swipeOffset: CGFloat = 0
    @State private var swipeHorizontal: Bool? = nil
    @State private var swipePastThreshold = false
    @State private var actionsRevealed = false
    @State private var showMovePicker = false
    @State private var confirmDelete = false
    @State private var showCreateFile = false
    @State private var showShare = false
    @State private var showRename = false

    private static let actionsWidth: CGFloat = 168

    var isSelected: Bool {
        selection.selectedIds.contains(id)
    }

    var file: File? {
        filesModel.idsToFiles[id]
    }

    var isPinned: Bool {
        file.map { filesModel.pinnedIds.contains($0.id) } == true
    }

    func body(content: Content) -> some View {
        macDraggable(
            swipeable(
                content
                    .background {
                        if isSelected {
                            RoundedRectangle(cornerRadius: 6, style: .continuous)
                                .fill(Color.accentColor.opacity(0.12))
                        }
                    }
            )
            .onTapGesture {
                handleTap()
            }
            #if os(iOS)
                .simultaneousGesture(
                    TapGesture(count: 2).onEnded {
                        openInNewWindow()
                    }
                )
            #endif
        )
        .contextMenu {
            if selection.selecting {
                contextMenuItem("Done Selecting", systemImage: "checkmark.circle.fill") {
                    withAnimation {
                        selection.end()
                    }
                }
            } else {
                if file?.isFolder == true {
                    contextMenuItem("New File Here", systemImage: "square.and.pencil") {
                        showCreateFile = true
                    }
                }

                if file?.isFolder == false, supportsMultipleWindows {
                    contextMenuItem("Open in New Window", systemImage: "macwindow.badge.plus") {
                        openInNewWindow()
                    }

                    Divider()
                }

                contextMenuItem("Rename", systemImage: "pencil") {
                    showRename = true
                }

                contextMenuItem("Move", systemImage: "folder") {
                    showMovePicker = true
                }

                contextMenuItem(
                    isPinned ? "Unpin" : "Pin",
                    systemImage: isPinned ? "pin.slash" : "pin"
                ) {
                    if let file {
                        filesModel.togglePin(file.id)
                    }
                }

                contextMenuItem("Share", systemImage: "square.and.arrow.up") {
                    showShare = true
                }

                contextMenuItem("Select", systemImage: "checkmark.circle") {
                    withAnimation {
                        selection.toggle(id)
                    }
                }

                Divider()

                contextMenuItem("Delete", systemImage: "trash", role: .destructive) {
                    confirmDelete = true
                }
            }
        }
        .sheet(isPresented: $showCreateFile) {
            CreateFileSheet(
                fileTreeModel: fileTreeModel,
                mode: .create(location: file.map(LocationChoice.init) ?? .root)
            )
        }
        .sheet(isPresented: $showRename) {
            if let file {
                CreateFileSheet(fileTreeModel: fileTreeModel, mode: .rename(file))
            }
        }
        .sheet(isPresented: $showShare) {
            ShareFileSheet(id: id)
        }
        .sheet(isPresented: $showMovePicker) {
            FolderPickerSheet(fileTreeModel: fileTreeModel, selection: nil) { folder in
                guard let file else {
                    return
                }

                _ = fileTreeModel.moveAndReveal([file], into: folder)
            }
        }
        .confirmationDialog(
            "Delete \"\(file?.name ?? "")\"? This cannot be undone.",
            isPresented: $confirmDelete,
            titleVisibility: .visible
        ) {
            Button("Delete", role: .destructive) {
                deleteFile()
            }
        }
    }

    @ViewBuilder
    private func macDraggable(_ content: some View) -> some View {
        #if os(macOS)
            content.overlay {
                if let file {
                    MacFileDragSource(file: file, filesModel: filesModel, onClick: {
                        handleTap()
                    }, onDoubleClick: {
                        openInNewWindow()
                    })
                }
            }
        #else
            content
        #endif
    }

    private var supportsMultipleWindows: Bool {
        #if os(iOS)
            UIApplication.shared.supportsMultipleScenes
        #else
            true
        #endif
    }

    private func openInNewWindow() {
        guard let file, !file.isFolder, !selection.selecting, supportsMultipleWindows else {
            return
        }

        openWindow(id: documentWindowId, value: file.id)
    }

    @ViewBuilder
    private func swipeable(_ content: some View) -> some View {
        #if os(iOS)
            content
                .offset(x: swipeOffset)
                .background(alignment: .leading) {
                    if swipeOffset > 0 {
                        selectRevealIndicator(offset: swipeOffset)
                    }
                }
                .background(alignment: .trailing) {
                    if swipeOffset < 0 {
                        trailingActions
                            .frame(width: -swipeOffset)
                            .clipShape(RoundedRectangle(cornerRadius: 6, style: .continuous))
                    }
                }
                .sensoryFeedback(.selection, trigger: swipePastThreshold)
                .simultaneousGesture(swipeGesture)
        #else
            content
        #endif
    }

    private var trailingActions: some View {
        HStack(spacing: 0) {
            swipeAction("folder.fill", .purple) {
                closeActions()
                showMovePicker = true
            }

            swipeAction(isPinned ? "pin.slash.fill" : "pin.fill", .orange) {
                if let file {
                    filesModel.togglePin(file.id)
                }
                closeActions()
            }

            swipeAction("trash.fill", .red) {
                confirmDelete = true
            }
        }
    }

    private func swipeAction(
        _ icon: String, _ color: Color, action: @escaping () -> Void
    ) -> some View {
        Button(action: action) {
            color.overlay {
                Image(systemName: icon)
                    .font(.system(size: 17))
                    .foregroundStyle(.white)
            }
        }
        .buttonStyle(.plain)
    }

    private var swipeGesture: some Gesture {
        DragGesture(minimumDistance: 20)
            .onChanged { value in
                if swipeHorizontal == nil {
                    swipeHorizontal =
                        abs(value.translation.width) > abs(value.translation.height)
                }

                guard swipeHorizontal == true else { return }

                let base: CGFloat = actionsRevealed ? -Self.actionsWidth : 0
                let raw = base + value.translation.width

                if raw >= 0 {
                    swipeOffset = rubberBand(raw, threshold: swipeThreshold)
                    swipePastThreshold = swipeOffset >= swipeThreshold
                } else {
                    swipeOffset =
                        raw > -Self.actionsWidth
                            ? raw
                            : -Self.actionsWidth + (raw + Self.actionsWidth) * 0.2
                    swipePastThreshold = false
                }
            }
            .onEnded { _ in
                defer { swipeHorizontal = nil }
                guard swipeHorizontal == true else { return }

                if swipePastThreshold {
                    selection.toggleAndCollapse(id)
                    swipePastThreshold = false
                    closeActions()
                } else if swipeOffset < -Self.actionsWidth / 2 {
                    actionsRevealed = true
                    withAnimation(.spring(response: 0.3, dampingFraction: 0.8)) {
                        swipeOffset = -Self.actionsWidth
                    }
                } else {
                    closeActions()
                }
            }
    }

    private func closeActions() {
        actionsRevealed = false
        withAnimation(.spring(response: 0.3, dampingFraction: 0.8)) {
            swipeOffset = 0
        }
    }

    private func deleteFile() {
        guard let file else {
            return
        }

        filesModel.deleteFiles([file], notifying: workspaceInput)
        closeActions()
    }

    private func handleTap() {
        if actionsRevealed {
            closeActions()
            return
        }

        selection.handleTap(id: id, orderedIds: orderedIds, open: open)
    }
}

struct SelectSwipeModifier: ViewModifier {
    let selection: SelectionModel
    let id: UUID

    @State private var swipeOffset: CGFloat = 0
    @State private var swipeHorizontal: Bool? = nil
    @State private var swipePastThreshold = false

    func body(content: Content) -> some View {
        #if os(iOS)
            content
                .offset(x: swipeOffset)
                .background(alignment: .leading) {
                    if swipeOffset > 0 {
                        selectRevealIndicator(offset: swipeOffset)
                    }
                }
                .sensoryFeedback(.selection, trigger: swipePastThreshold)
                .simultaneousGesture(swipeGesture)
        #else
            content
        #endif
    }

    private var swipeGesture: some Gesture {
        DragGesture(minimumDistance: 20)
            .onChanged { value in
                if swipeHorizontal == nil {
                    swipeHorizontal =
                        abs(value.translation.width) > abs(value.translation.height)
                }

                guard swipeHorizontal == true else { return }

                let width = max(value.translation.width, 0)
                swipeOffset = rubberBand(width, threshold: swipeThreshold)
                swipePastThreshold = swipeOffset >= swipeThreshold
            }
            .onEnded { _ in
                defer { swipeHorizontal = nil }
                guard swipeHorizontal == true else { return }

                if swipePastThreshold {
                    selection.toggleAndCollapse(id)
                }

                swipePastThreshold = false

                withAnimation(.spring(response: 0.3, dampingFraction: 0.8)) {
                    swipeOffset = 0
                }
            }
    }
}

struct SelectionCommandsModifier: ViewModifier {
    @Environment(FilesModel.self) private var filesModel
    @Environment(FileTreeModel.self) private var fileTreeModel
    @Environment(WorkspaceInputState.self) private var workspaceInput

    let selection: SelectionModel

    @State private var confirmDelete = false
    @State private var showMovePicker = false

    func body(content: Content) -> some View {
        content
            .toolbar {
                if selection.selecting {
                    ToolbarItem(placement: donePlacement) {
                        Button("Done") {
                            withAnimation {
                                selection.end()
                            }
                        }
                        .keyboardShortcut(.cancelAction)
                    }
                }
            }
            .safeAreaInset(edge: .bottom) {
                if selection.selecting {
                    commandBar
                }
            }
            .sheet(isPresented: $showMovePicker) {
                FolderPickerSheet(fileTreeModel: fileTreeModel, selection: nil) { folder in
                    moveSelectedFiles(into: folder)
                }
            }
    }

    private var commandBar: some View {
        HStack(spacing: 10) {
            moveButton
            deleteBar
        }
        .padding(.bottom, 10)
        .transition(.move(edge: .bottom).combined(with: .opacity))
    }

    private var moveButton: some View {
        Button {
            showMovePicker = true
        } label: {
            Label("Move", systemImage: "folder")
        }
        .buttonStyle(.glassProminent)
        .disabled(selectedFiles.isEmpty)
    }

    private func moveSelectedFiles(into folder: File) {
        let files = selectedFiles
        guard !files.isEmpty, fileTreeModel.moveAndReveal(files, into: folder) else {
            return
        }

        withAnimation {
            selection.end()
        }
    }

    private var donePlacement: ToolbarItemPlacement {
        #if os(iOS)
            .topBarTrailing
        #else
            .automatic
        #endif
    }

    private var deleteBar: some View {
        Button(role: .destructive) {
            confirmDelete = true
        } label: {
            Label(deleteCountLabel, systemImage: "trash")
        }
        .buttonStyle(.glassProminent)
        .tint(.red)
        .disabled(selectedFiles.isEmpty)
        .keyboardShortcut(.delete, modifiers: .command)
        .confirmationDialog(
            "\(deleteCountLabel)? This cannot be undone.",
            isPresented: $confirmDelete,
            titleVisibility: .visible
        ) {
            Button("Delete", role: .destructive) {
                deleteSelectedFiles()
            }
        }
    }

    private var selectedFiles: [File] {
        selection.selectedFiles(in: filesModel)
    }

    private var deleteCountLabel: String {
        let count = selectedFiles.count
        return count == 1 ? "Delete 1 file" : "Delete \(count) files"
    }

    private func deleteSelectedFiles() {
        let files = selectedFiles
        guard !files.isEmpty else {
            return
        }

        filesModel.deleteFiles(files, notifying: workspaceInput)

        withAnimation {
            selection.end()
        }
    }
}

extension View {
    func selectableRow(
        _ selection: SelectionModel,
        id: UUID,
        orderedIds: @escaping () -> [UUID],
        open: @escaping () -> Void
    ) -> some View {
        modifier(
            SelectableRowModifier(
                selection: selection,
                id: id,
                orderedIds: orderedIds,
                open: open
            )
        )
    }

    func selectSwipe(_ selection: SelectionModel, id: UUID) -> some View {
        modifier(SelectSwipeModifier(selection: selection, id: id))
    }

    func selectionCommands(_ selection: SelectionModel) -> some View {
        modifier(SelectionCommandsModifier(selection: selection))
    }
}
