import SwiftUI
import SwiftWorkspace

struct FileTreeView: View {
    @Environment(FilesModel.self) private var filesModel
    @Environment(WorkspaceInputState.self) private var workspaceInput
    @Environment(WorkspaceOutputState.self) private var workspaceOutput

    let fileTreeModel: FileTreeModel

    @State private var isRootDropTargeted = false
    @State private var topZoneTargeted = false
    @State private var bottomZoneTargeted = false
    @State private var showAutoScrollZones = false
    @State private var atTop = true
    @State private var atBottom = false
    @State private var zoneDecayTask: Task<Void, Never>?
    @State private var stickyHeaders: [File] = []
    @State private var showRootCreate = false
    @State private var dragScrollTop: CGFloat = 0
    @State private var scrollPosition = ScrollPosition()
    @State private var autoScroll = AutoScrollDriver()

    var body: some View {
        if let root = filesModel.root {
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 2) {
                    ForEach(fileTreeModel.visibleRows) { row in
                        FileRowView(file: row.file, level: row.level)
                    }
                }
                .scrollTargetLayout()
                .padding(.horizontal)
            }
            .contextMenu {
                contextMenuItem("New File", systemImage: "square.and.pencil") {
                    showRootCreate = true
                }
            }
            .sheet(isPresented: $showRootCreate) {
                CreateFileSheet(fileTreeModel: fileTreeModel)
            }
            .scrollPosition($scrollPosition)
            .onChange(of: workspaceOutput.openDoc) { _, openDoc in
                guard let openDoc else { return }
                Task {
                    withAnimation {
                        scrollPosition.scrollTo(id: openDoc, anchor: .center)
                    }
                }
            }
            .onScrollGeometryChange(for: ScrollGeometry.self, of: { $0 }) { _, newValue in
                autoScroll.scroll = newValue

                let top = newValue.contentOffset.y + newValue.contentInsets.top
                let maxOffset = max(
                    0,
                    newValue.contentSize.height + newValue.contentInsets.top + newValue.contentInsets.bottom
                        - newValue.containerSize.height
                )
                withAnimation(.easeInOut(duration: 0.15)) {
                    atTop = top <= 2
                    atBottom = top >= maxOffset - 2
                }

                if fileTreeModel.dropTarget != nil {
                    dragScrollTop = top
                }
            }
            .onScrollTargetVisibilityChange(idType: UUID.self, threshold: 0.3) { visibleIds in
                updateStickyHeaders(visibleIds: visibleIds)
            }
            .onChange(of: fileTreeModel.dropTarget) { _, target in
                if target != nil, let scroll = autoScroll.scroll {
                    dragScrollTop = scroll.contentOffset.y + scroll.contentInsets.top
                }
                updateDragActivity()
            }
            .onDisappear {
                stopAutoScroll()
                zoneDecayTask?.cancel()
                fileTreeModel.dropTarget = nil
            }
            .fileDropTarget(isTargeted: { targeted in
                withAnimation(.easeInOut(duration: 0.12)) {
                    isRootDropTargeted = targeted
                }
                updateDragActivity()
            }, action: { dropped in
                filesModel.drop(dropped, into: root)
            })
            .overlay(alignment: .top) {
                if let target = fileTreeModel.dropTarget, target.isFolder {
                    dropRegionIndicator(for: target)
                }
            }
            .overlay {
                if isRootDropTargeted {
                    RoundedRectangle(cornerRadius: 8, style: .continuous)
                        .strokeBorder(Color.accentColor, lineWidth: 2)
                        .padding(4)
                }
            }
            .overlay(alignment: .top) {
                if !stickyHeaders.isEmpty, !showAutoScrollZones {
                    stickyHeaderStack(stickyHeaders)
                }
            }
            .overlay(alignment: .top) {
                if showAutoScrollZones, !atTop {
                    autoScrollZone(direction: -1)
                }
            }
            .overlay(alignment: .bottom) {
                if showAutoScrollZones, !atBottom {
                    autoScrollZone(direction: 1)
                }
            }
            .refreshable {
                workspaceInput.requestSync()
            }
            .selectionCommands(fileTreeModel.selection, fileTreeModel: fileTreeModel)
            .environment(fileTreeModel)
            .navigationTitle(root.name)
            #if os(iOS)
                .navigationBarTitleDisplayMode(.large)
            #endif
        } else {
            ProgressView()
        }
    }

    @ViewBuilder
    private func dropRegionIndicator(for folder: File) -> some View {
        if let region = dropRegionFrame(for: folder) {
            RoundedRectangle(cornerRadius: 5, style: .continuous)
                .fill(Color.accentColor.opacity(0.08))
                .overlay {
                    RoundedRectangle(cornerRadius: 5, style: .continuous)
                        .strokeBorder(Color.accentColor.opacity(0.7), lineWidth: 1.5)
                }
                .frame(height: region.height)
                .padding(.horizontal, 8)
                .offset(y: region.minY)
                .allowsHitTesting(false)
        }
    }

    private func dropRegionFrame(for folder: File) -> (minY: CGFloat, height: CGFloat)? {
        let rows = fileTreeModel.visibleRows
        guard let scroll = autoScroll.scroll,
              let start = rows.firstIndex(where: { $0.id == folder.id })
        else {
            return nil
        }

        var end = start
        while end + 1 < rows.count, rows[end + 1].level > rows[start].level {
            end += 1
        }

        let pitch = max(1, scroll.contentSize.height / CGFloat(rows.count))
        let minY = max(CGFloat(start) * pitch - dragScrollTop, 0)
        let maxY = min(
            CGFloat(end + 1) * pitch - dragScrollTop,
            scroll.containerSize.height - scroll.contentInsets.top - scroll.contentInsets.bottom
        )

        guard maxY > minY else {
            return nil
        }

        return (minY, maxY - minY)
    }

    #if os(iOS)
        private static let minStickyBaseIndex = 2
    #else
        private static let minStickyBaseIndex = 1
    #endif

    private func updateStickyHeaders(visibleIds: [UUID]) {
        let visible = Set(visibleIds)

        var headers: [File] = []
        if let baseIndex = fileTreeModel.visibleRows.firstIndex(where: { visible.contains($0.id) }),
           baseIndex >= Self.minStickyBaseIndex
        {
            headers = stickyChain(fromRowAt: baseIndex)
        }

        if stickyHeaders != headers {
            withAnimation(.easeInOut(duration: 0.15)) {
                stickyHeaders = headers
            }
        }
    }

    private func stickyChain(fromRowAt baseIndex: Int) -> [File] {
        let rows = fileTreeModel.visibleRows
        guard !rows.isEmpty else {
            return []
        }

        func row(_ index: Int) -> FileTreeRow {
            rows[min(max(index, 0), rows.count - 1)]
        }

        var headers: [File] = []

        for offset in 0 ..< 32 {
            headers = ancestors(of: row(baseIndex + offset).file)

            if headers.count <= offset {
                break
            }
        }

        if headers.isEmpty {
            headers = ancestors(of: row(baseIndex).file)
        }

        return headers
    }

    private func ancestors(of file: File) -> [File] {
        var chain: [File] = []
        var current = file

        while let parent = filesModel.idsToFiles[current.parent], !parent.isRoot, parent.id != current.id {
            chain.append(parent)
            current = parent
        }

        return chain.reversed()
    }

    private func stickyHeaderStack(_ files: [File]) -> some View {
        VStack(spacing: 0) {
            ForEach(Array(files.enumerated()), id: \.element.id) { level, file in
                stickyHeaderRow(file, level: level)
            }
        }
        .background(.bar)
        .overlay(alignment: .bottom) {
            Divider()
        }
        .transition(.opacity)
    }

    private func stickyHeaderRow(_ file: File, level: Int) -> some View {
        Button {
            scrollToRevealBelowHeaders(file, headerCount: level)
        } label: {
            HStack {
                Image(systemName: FileIconHelper.fileToSystemImageName(file: file))
                    .font(.system(size: 16))
                    .frame(width: 16)
                    .foregroundColor(.accentColor)

                Text(file.name)
                    .lineLimit(1)
                    .truncationMode(.tail)
                    .foregroundColor(.primary)

                Spacer()
            }
            .padding(.vertical, 9)
            .padding(.leading, CGFloat(level) * 20)
            .padding(.horizontal)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
    }

    private func scrollToRevealBelowHeaders(_ file: File, headerCount: Int) {
        let rows = fileTreeModel.visibleRows
        guard let index = rows.firstIndex(where: { $0.id == file.id }) else {
            return
        }

        var anchorIndex = max(index - headerCount, 0)
        while anchorIndex > 0, stickyChain(fromRowAt: anchorIndex).count > index - anchorIndex {
            anchorIndex -= 1
        }

        withAnimation {
            scrollPosition.scrollTo(id: rows[anchorIndex].id, anchor: .top)
        }
    }

    private func autoScrollZone(direction: Int) -> some View {
        ZStack {
            LinearGradient(
                colors: [Color.accentColor.opacity(0.15), Color.accentColor.opacity(0)],
                startPoint: direction < 0 ? .top : .bottom,
                endPoint: direction < 0 ? .bottom : .top
            )

            Image(systemName: direction < 0 ? "chevron.compact.up" : "chevron.compact.down")
                .font(.system(size: 20, weight: .semibold))
                .foregroundStyle(Color.accentColor.opacity(0.8))
        }
        .frame(height: 40)
        .frame(maxWidth: .infinity)
        .contentShape(Rectangle())
        .fileDropTarget(isTargeted: { targeted in
            setZoneTargeted(direction, targeted)
        }, action: { _ in
            false
        })
        .onDisappear {
            setZoneTargeted(direction, false)
        }
        .transition(.opacity)
    }

    private func updateDragActivity() {
        let active = isRootDropTargeted || topZoneTargeted || bottomZoneTargeted
            || fileTreeModel.dropTarget != nil

        zoneDecayTask?.cancel()
        zoneDecayTask = nil

        if active {
            if !showAutoScrollZones {
                withAnimation(.easeInOut(duration: 0.15)) {
                    showAutoScrollZones = true
                }
            }
        } else {
            zoneDecayTask = Task {
                try? await Task.sleep(for: .milliseconds(300))
                guard !Task.isCancelled else { return }

                withAnimation(.easeInOut(duration: 0.15)) {
                    showAutoScrollZones = false
                }
            }
        }
    }

    private func setZoneTargeted(_ direction: Int, _ targeted: Bool) {
        if direction < 0 {
            topZoneTargeted = targeted
        } else {
            bottomZoneTargeted = targeted
        }
        updateDragActivity()

        if targeted {
            autoScroll.direction = direction
            startAutoScroll()
        } else if direction == autoScroll.direction {
            stopAutoScroll()
        }
    }

    private func startAutoScroll() {
        stopAutoScroll()

        autoScroll.rowIndex = estimatedEdgeRowIndex(autoScroll.direction)

        let timer = Timer(timeInterval: 0.12, repeats: true) { _ in
            stepScroll()
        }
        RunLoop.main.add(timer, forMode: .common)
        autoScroll.timer = timer
    }

    private func stopAutoScroll() {
        autoScroll.timer?.invalidate()
        autoScroll.timer = nil
    }

    private func estimatedEdgeRowIndex(_ direction: Int) -> Int {
        let rows = fileTreeModel.visibleRows
        guard let scroll = autoScroll.scroll, !rows.isEmpty else {
            return 0
        }

        let pitch = max(1, scroll.contentSize.height / CGFloat(rows.count))
        let top = scroll.contentOffset.y + scroll.contentInsets.top
        let visibleHeight = scroll.containerSize.height - scroll.contentInsets.top - scroll.contentInsets.bottom
        let edgeY = direction > 0 ? top + visibleHeight : top
        let index = Int(edgeY / pitch)

        return min(max(index, 0), rows.count - 1)
    }

    private func stepScroll() {
        let direction = autoScroll.direction
        let zoneActive = direction < 0 ? topZoneTargeted : bottomZoneTargeted
        let rows = fileTreeModel.visibleRows

        guard zoneActive, !rows.isEmpty else {
            stopAutoScroll()
            return
        }

        autoScroll.rowIndex = min(max(autoScroll.rowIndex + direction, 0), rows.count - 1)
        let row = rows[autoScroll.rowIndex]

        withAnimation(.linear(duration: 0.12)) {
            scrollPosition.scrollTo(id: row.id, anchor: direction > 0 ? .bottom : .top)
        }
    }
}

final class AutoScrollDriver {
    var scroll: ScrollGeometry?
    var timer: Timer?
    var direction: Int = 1
    var rowIndex: Int = 0
}

struct FileRowView: View {
    @Environment(FilesModel.self) private var filesModel
    @Environment(FileTreeModel.self) private var fileTreeModel
    @Environment(HomeState.self) private var homeState
    @Environment(WorkspaceInputState.self) private var workspaceInput

    let file: File
    let level: CGFloat

    @State private var isDropTargeted = false
    @State private var springOpenTask: Task<Void, Never>?

    var isLeaf: Bool {
        (filesModel.childrenByParent[file.id] ?? []).isEmpty
    }

    var isOpen: Bool {
        fileTreeModel.openFolders.contains(file.id)
    }

    var body: some View {
        fileRow
            .selectableRow(
                fileTreeModel.selection,
                id: file.id,
                fileTreeModel: fileTreeModel,
                orderedIds: { fileTreeModel.visibleRows.map(\.id) },
                open: openFile
            )
            #if os(iOS)
                .draggable(DraggedFile(file: file, filesModel: filesModel))
            #endif
            .fileDropTarget(isTargeted: { targeted in
                setDropTargeted(targeted)
            }, action: { dropped in
                drop(dropped)
            })
    }

    var fileRow: some View {
        HStack {
            SelectionIndicator(selection: fileTreeModel.selection, id: file.id)

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

            if filesModel.pinnedIds.contains(file.id) {
                Image(systemName: "pin.fill")
                    .font(.system(size: 10))
                    .foregroundStyle(.orange)
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
        .overlay(alignment: .bottom) {
            if isDropTargeted, !file.isFolder {
                Capsule()
                    .fill(Color.accentColor)
                    .frame(height: 2.5)
                    .padding(.leading, level * 20 + 5)
                    .padding(.trailing, 10)
            }
        }
    }

    func openFile() {
        if file.isFolder {
            workspaceInput.selectFolder(id: file.id)

            withAnimation {
                fileTreeModel.toggleFolder(file.id)
            }
        } else {
            workspaceInput.openFile(id: file.id)
            homeState.compactColumn = .detail
        }
    }

    private func setDropTargeted(_ targeted: Bool) {
        withAnimation(.easeInOut(duration: 0.12)) {
            isDropTargeted = targeted
        }

        if targeted {
            fileTreeModel.dropTarget = file
        } else if fileTreeModel.dropTarget == file {
            fileTreeModel.dropTarget = nil
        }

        springOpenTask?.cancel()
        springOpenTask = nil

        if targeted, file.isFolder, !isOpen {
            springOpenTask = Task {
                try? await Task.sleep(for: .milliseconds(650))
                guard !Task.isCancelled else { return }

                withAnimation {
                    fileTreeModel.openFolders.insert(file.id)
                }
            }
        }
    }

    private var dropDestinationFolder: File? {
        file.isFolder ? file : filesModel.idsToFiles[file.parent]
    }

    private func drop(_ dropped: [FileDropItem]) -> Bool {
        guard let dest = dropDestinationFolder, filesModel.drop(dropped, into: dest) else {
            return false
        }

        withAnimation {
            fileTreeModel.openFolders.insert(dest.id)
        }

        return true
    }
}

#Preview {
    let filesModel = FilesModel.preview

    NavigationStack {
        FileTreeView(fileTreeModel: FileTreeModel(filesModel: filesModel))
    }
    .environment(filesModel)
    .environment(HomeState())
    .environment(WorkspaceInputState.preview)
    .environment(WorkspaceOutputState.preview)
}
