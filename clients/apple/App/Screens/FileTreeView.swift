import SwiftUI
import SwiftWorkspace

struct FileTreeView: View {
    @Environment(FilesModel.self) private var filesModel
    @Environment(WorkspaceInputState.self) private var workspaceInput
    @Environment(WorkspaceOutputState.self) private var workspaceOutput
    #if os(iOS)
        @Environment(HomeState.self) private var homeState
    #endif

    let fileTreeModel: FileTreeModel

    #if os(iOS)
        @Environment(\.horizontalSizeClass) private var horizontalSizeClass
    #endif
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
                if fileTreeModel.visibleRows.isEmpty {
                    emptyState
                } else {
                    LazyVStack(alignment: .leading, spacing: 2) {
                        ForEach(fileTreeModel.visibleRows) { row in
                            FileRowView(file: row.file, level: row.level)
                        }
                    }
                    .scrollTargetLayout()
                    .padding(.horizontal)

                    if showStartHereHint {
                        startHereHint
                    }
                }
            }
            .overlay(alignment: CreateButtonArrow.alignment) {
                if fileTreeModel.visibleRows.isEmpty {
                    CreateButtonArrow()
                }
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
            .onAppear {
                scrollToOpenDoc()
            }
            .onChange(of: workspaceOutput.openDoc) {
                scrollToOpenDoc()
            }
            .onScrollGeometryChange(for: ScrollGeometry.self, of: { $0 }) { _, newValue in
                autoScroll.scroll = newValue

                let top = newValue.topOffset
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
                    dragScrollTop = scroll.topOffset
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
                #if os(iOS)
                    homeState.explicitSyncRequested()
                #endif
                workspaceInput.requestSync()
            }
            .selectionCommands(fileTreeModel.selection)
            .environment(fileTreeModel)
            .navigationTitle(root.name)
            .largeNavigationTitle()
        } else {
            ProgressView()
        }
    }

    private var showStartHereHint: Bool {
        AppState.shared.accountCreatedThisSession
    }

    private var startHereHint: some View {
        StartHereHint(leadingInset: 21)
            .padding(.top, -10)
    }

    private var emptyState: some View {
        VStack(spacing: 12) {
            Image(systemName: "sparkles")
                .font(.system(size: 34))
                .foregroundStyle(Color.accentColor.opacity(0.5))

            Text("Nothing here yet")
                .font(.headline)

            Text(emptySubtext)
                .font(.callout)
                .foregroundStyle(.secondary)
                .multilineTextAlignment(.center)
        }
        .frame(maxWidth: .infinity)
        .padding(.horizontal, 32)
        .containerRelativeFrame(.vertical) { height, _ in height * 0.6 }
    }

    private var emptySubtext: String {
        #if os(iOS)
            if horizontalSizeClass == .compact {
                "Notes, drawings, folders — create your first below."
            } else {
                "Tap the pencil in the corner to create your first note."
            }
        #else
            "Notes, drawings, folders — create your first with New."
        #endif
    }

    private func scrollToOpenDoc() {
        guard let openDoc = workspaceOutput.openDoc else { return }
        Task {
            withAnimation {
                scrollPosition.scrollTo(id: openDoc, anchor: .center)
            }
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

        let pitch = scroll.rowPitch(rowCount: rows.count)
        let minY = max(CGFloat(start) * pitch - dragScrollTop, 0)
        let maxY = min(CGFloat(end + 1) * pitch - dragScrollTop, scroll.visibleHeight)

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
            headers = filesModel.ancestors(of: row(baseIndex + offset).file)

            if headers.count <= offset {
                break
            }
        }

        if headers.isEmpty {
            headers = filesModel.ancestors(of: row(baseIndex).file)
        }

        return headers
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

        let pitch = scroll.rowPitch(rowCount: rows.count)
        let top = scroll.topOffset
        let edgeY = direction > 0 ? top + scroll.visibleHeight : top
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

private extension ScrollGeometry {
    var topOffset: CGFloat {
        contentOffset.y + contentInsets.top
    }

    var visibleHeight: CGFloat {
        containerSize.height - contentInsets.top - contentInsets.bottom
    }

    func rowPitch(rowCount: Int) -> CGFloat {
        max(1, contentSize.height / CGFloat(rowCount))
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
