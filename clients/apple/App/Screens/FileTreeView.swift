import SwiftUI
import SwiftWorkspace

struct FileTreeView: View {
    @Environment(FilesModel.self) private var filesModel
    @Environment(WorkspaceInputState.self) private var workspaceInput
    @Environment(WorkspaceOutputState.self) private var workspaceOutput

    let fileTreeModel: FileTreeModel

    @State private var isRootDropTargeted = false
    @State private var isRowDropTargeted = false
    @State private var topZoneTargeted = false
    @State private var bottomZoneTargeted = false
    @State private var showAutoScrollZones = false
    @State private var atTop = true
    @State private var atBottom = false
    @State private var zoneDecayTask: Task<Void, Never>?
    @State private var stickyHeaders: [File] = []
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
            .scrollPosition($scrollPosition)
            .onChange(of: workspaceOutput.openDoc) {
                guard let openDoc = workspaceOutput.openDoc else { return }
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

                updateStickyHeaders()
            }
            .onPreferenceChange(DropTargetActiveKey.self) { targeted in
                isRowDropTargeted = targeted
                updateDragActivity()
            }
            .onDisappear {
                stopAutoScroll()
                zoneDecayTask?.cancel()
            }
            .dropDestination(for: File.self) { dropped, _ in
                moveToRoot(dropped, root: root)
            } isTargeted: { targeted in
                withAnimation(.easeInOut(duration: 0.12)) {
                    isRootDropTargeted = targeted
                }
                updateDragActivity()
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
            .environment(fileTreeModel)
            .navigationTitle(root.name)
            #if os(iOS)
                .navigationBarTitleDisplayMode(.large)
            #endif
        } else {
            ProgressView()
        }
    }

    private func moveToRoot(_ dropped: [File], root: File) -> Bool {
        let movable = dropped.filter { filesModel.canMove($0, into: root) }
        guard !movable.isEmpty else {
            return false
        }

        for file in movable {
            filesModel.moveFile(id: file.id, newParent: root.id)
        }

        return true
    }

    private func updateStickyHeaders() {
        let rows = fileTreeModel.visibleRows
        var headers: [File] = []

        if let scroll = autoScroll.scroll, !rows.isEmpty {
            let top = scroll.contentOffset.y + scroll.contentInsets.top

            if top > 2 {
                let pitch = max(1, scroll.contentSize.height / CGFloat(rows.count))
                let baseIndex = Int(top / pitch)

                for offset in 0 ..< 32 {
                    let index = min(max(baseIndex + offset, 0), rows.count - 1)
                    headers = ancestors(of: rows[index].file)

                    if headers.count <= offset {
                        break
                    }
                }
            }
        }

        if stickyHeaders != headers {
            withAnimation(.easeInOut(duration: 0.15)) {
                stickyHeaders = headers
            }
        }
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
                stickyHeaderRow(file, level: CGFloat(level))
            }
        }
        .background(.bar)
        .overlay(alignment: .bottom) {
            Divider()
        }
        .transition(.opacity)
    }

    private func stickyHeaderRow(_ file: File, level: CGFloat) -> some View {
        Button {
            withAnimation {
                scrollPosition.scrollTo(id: file.id, anchor: .top)
            }
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
            .padding(.leading, level * 20)
            .padding(.horizontal)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
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
        .dropDestination(for: File.self) { _, _ in
            false
        } isTargeted: { targeted in
            setZoneTargeted(direction, targeted)
        }
        .transition(.opacity)
    }

    private func updateDragActivity() {
        let active = isRootDropTargeted || isRowDropTargeted || topZoneTargeted || bottomZoneTargeted

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

struct DropTargetActiveKey: PreferenceKey {
    static let defaultValue = false

    static func reduce(value: inout Bool, nextValue: () -> Bool) {
        value = value || nextValue()
    }
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

    var children: [File] {
        filesModel.childrenByParent[file.id] ?? []
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
            .draggable(file)
            .dropDestination(for: File.self) { dropped, _ in
                drop(dropped)
            } isTargeted: { targeted in
                setDropTargeted(targeted)
            }
            .preference(key: DropTargetActiveKey.self, value: isDropTargeted)
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
        .background {
            if isDropTargeted {
                RoundedRectangle(cornerRadius: 5, style: .continuous)
                    .fill(Color.accentColor.opacity(0.35))
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

    private func drop(_ dropped: [File]) -> Bool {
        guard let dest = dropDestinationFolder else {
            return false
        }

        let movable = dropped.filter { filesModel.canMove($0, into: dest) }
        guard !movable.isEmpty else {
            return false
        }

        for dragged in movable {
            filesModel.moveFile(id: dragged.id, newParent: dest.id)
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
