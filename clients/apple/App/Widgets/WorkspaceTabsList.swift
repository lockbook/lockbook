import CoreTransferable
import SwiftUI
import SwiftWorkspace
import UniformTypeIdentifiers

extension UTType {
    static let lockbookTab = UTType(exportedAs: "net.lockbook.tab")
}

struct TabDragItem: Codable, Transferable {
    let id: UUID

    static var transferRepresentation: some TransferRepresentation {
        CodableRepresentation(contentType: .lockbookTab)
    }
}

struct WorkspaceTabsList: View {
    @Environment(FilesModel.self) private var filesModel
    @Environment(WorkspaceInputState.self) private var workspaceInput
    @Environment(WorkspaceOutputState.self) private var workspaceOutput
    @Environment(\.openWindow) private var openWindow

    let fileTreeModel: FileTreeModel

    @State private var tabs: [WorkspaceTabInfo] = []
    @State private var recentlyClosed: [WorkspaceTabInfo] = []
    @State private var selection = SelectionModel()
    @State private var dropTargetId: UUID? = nil
    @State private var endZoneTargeted = false
    @State private var renameTarget: File? = nil
    @State private var canNavBack = false
    @State private var canNavForward = false

    var body: some View {
        Group {
            if tabs.isEmpty, recentlyClosed.isEmpty {
                emptyTabs
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            } else {
                ScrollView {
                    VStack(alignment: .leading, spacing: 2) {
                        if tabs.isEmpty {
                            emptyTabs
                                .frame(maxWidth: .infinity)
                                .padding(.vertical, 28)
                        } else {
                            ForEach(tabs) { tab in
                                tabRow(tab)
                            }
                        }

                        endDropZone

                        if !recentlyClosed.isEmpty {
                            Text("Recently Closed")
                                .font(.caption)
                                .fontWeight(.medium)
                                .foregroundStyle(.secondary)
                                .padding(.top, 14)
                                .padding(.leading, 10)

                            ForEach(recentlyClosed) { tab in
                                recentlyClosedRow(tab)
                            }
                        }
                    }
                    .padding(.horizontal, 8)
                    .padding(.top, 4)
                }
            }
        }
        .safeAreaInset(edge: .top) {
            header
        }
        .safeAreaInset(edge: .bottom) {
            bottomBar
        }
        .onAppear(perform: refresh)
        .onChange(of: workspaceOutput.tabCount) {
            refresh()
        }
        .onChange(of: workspaceOutput.currentSession) {
            refresh()
        }
        .onChange(of: workspaceOutput.openDoc) {
            refresh()
        }
        .sheet(item: $renameTarget) { file in
            CreateFileSheet(fileTreeModel: fileTreeModel, mode: .rename(file))
        }
    }

    private var header: some View {
        HStack {
            Text("Tabs")
                .font(.headline)

            Spacer()

            if selection.selecting {
                Button("Done") {
                    withAnimation {
                        selection.end()
                    }
                }
                .keyboardShortcut(.cancelAction)
            } else {
                Button {
                    workspaceInput.navBack()
                } label: {
                    Image(systemName: "chevron.left")
                        .fontWeight(.semibold)
                }
                .disabled(!canNavBack)

                Button {
                    workspaceInput.navForward()
                } label: {
                    Image(systemName: "chevron.right")
                        .fontWeight(.semibold)
                }
                .disabled(!canNavForward)
            }
        }
        .padding(.horizontal)
        .padding(.vertical, 10)
        .background(.ultraThinMaterial)
    }

    @ViewBuilder
    private var bottomBar: some View {
        if selection.selecting {
            Button(role: .destructive) {
                closeSelected()
            } label: {
                Label(closeCountLabel, systemImage: "xmark")
            }
            .buttonStyle(.glassProminent)
            .tint(.red)
            .disabled(selection.selectedIds.isEmpty)
            .padding(.bottom, 10)
            .transition(.move(edge: .bottom).combined(with: .opacity))
        } else if !tabs.isEmpty {
            Button {
                workspaceInput.closeAllTabs()
            } label: {
                Text("Close All")
                    .frame(maxWidth: .infinity)
            }
            .buttonStyle(.bordered)
            .padding()
        }
    }

    private var emptyTabs: some View {
        VStack(spacing: 6) {
            Image(systemName: recentlyClosed.isEmpty ? "rectangle.on.rectangle" : "arrow.uturn.left")
                .font(.system(size: 30, weight: .light))
                .foregroundStyle(.tertiary)
                .padding(.bottom, 6)

            Text("No open tabs")
                .font(.callout)
                .fontWeight(.semibold)

            Text(
                recentlyClosed.isEmpty
                    ? "Files you open will appear here."
                    : "Reopen a recently closed tab below."
            )
            .font(.caption)
            .foregroundStyle(.secondary)
        }
        .multilineTextAlignment(.center)
        .padding(.horizontal, 20)
    }

    private func tabRow(_ tab: WorkspaceTabInfo) -> some View {
        let id = tab.id
        let isCurrent = id == workspaceOutput.currentSession
        let name = tabName(tab)

        return HStack(spacing: 8) {
            SelectionIndicator(selection: selection, id: id)

            Image(systemName: "line.3.horizontal")
                .font(.system(size: 12, weight: .semibold))
                .foregroundStyle(.tertiary)
                .padding(.vertical, 8)
                .padding(.trailing, 2)

            Image(systemName: tabIcon(tab, name: name))
                .foregroundStyle(isCurrent ? Color.accentColor : Color.secondary)

            Text(name)
                .lineLimit(1)
                .truncationMode(.tail)
                .onTapGesture {
                    selection.handleTap(id: id, orderedIds: { tabIds }) {
                        if isCurrent, let fileId = tab.destFile {
                            renameTarget = filesModel.idsToFiles[fileId]
                        } else {
                            workspaceInput.activateTab(id: id)
                        }
                    }
                }

            Spacer(minLength: 0)

            if !selection.selecting {
                Button {
                    workspaceInput.closeTab(id: id)
                } label: {
                    Image(systemName: "xmark")
                        .font(.system(size: 10, weight: .bold))
                        .foregroundStyle(.secondary)
                }
                .buttonStyle(.plain)
            }
        }
        .padding(.vertical, 6)
        .padding(.horizontal, 8)
        .background {
            if isCurrent {
                RoundedRectangle(cornerRadius: 6, style: .continuous)
                    .fill(Color.accentColor.opacity(0.15))
            }
        }
        .contentShape(Rectangle())
        .onTapGesture {
            selection.handleTap(id: id, orderedIds: { tabIds }) {
                workspaceInput.activateTab(id: id)
            }
        }
        .onMiddleClick {
            close([id])
        }
        .selectSwipe(selection, id: id)
        .contextMenu {
            tabMenu(tab)
        }
        .draggable(TabDragItem(id: id))
        .dropDestination(for: TabDragItem.self) { items, _ in
            dropTab(items.first, before: id)
        } isTargeted: { targeted in
            dropTargetId = targeted ? id : (dropTargetId == id ? nil : dropTargetId)
        }
        .overlay(alignment: .top) {
            if dropTargetId == id {
                Capsule()
                    .fill(Color.accentColor)
                    .frame(height: 2.5)
            }
        }
    }

    private var endDropZone: some View {
        Color.clear
            .frame(height: 12)
            .frame(maxWidth: .infinity)
            .contentShape(Rectangle())
            .dropDestination(for: TabDragItem.self) { items, _ in
                dropTab(items.first, before: nil)
            } isTargeted: { targeted in
                endZoneTargeted = targeted
            }
            .overlay(alignment: .top) {
                if endZoneTargeted {
                    Capsule()
                        .fill(Color.accentColor)
                        .frame(height: 2.5)
                }
            }
    }

    @ViewBuilder
    private func tabMenu(_ tab: WorkspaceTabInfo) -> some View {
        let id = tab.id
        if selection.selecting {
            contextMenuItem("Done Selecting", systemImage: "checkmark.circle.fill") {
                withAnimation {
                    selection.end()
                }
            }
        } else {
            if supportsMultipleWindows, let fileId = tab.destFile, tab.destKind == .file {
                contextMenuItem("Open in New Window", systemImage: "macwindow.badge.plus") {
                    openWindow(id: documentWindowId, value: fileId)
                }

                Divider()
            }

            contextMenuItem("Close", systemImage: "xmark") {
                close([id])
            }

            contextMenuItem("Close Others", systemImage: "xmark.square") {
                close(tabIds.filter { $0 != id })
            }
            .disabled(tabs.count < 2)

            contextMenuItem("Close Above", systemImage: "arrow.up.to.line") {
                closeAbove(id)
            }
            .disabled(tabs.first?.id == id)

            contextMenuItem("Close Below", systemImage: "arrow.down.to.line") {
                closeBelow(id)
            }
            .disabled(tabs.last?.id == id)

            contextMenuItem("Close All", systemImage: "xmark.circle") {
                workspaceInput.closeAllTabs()
            }

            Divider()

            if let fileId = tab.destFile, tab.destKind == .file {
                contextMenuItem("Rename", systemImage: "pencil") {
                    renameTarget = filesModel.idsToFiles[fileId]
                }
            }

            contextMenuItem("Reopen Closed Tab", systemImage: "arrow.uturn.left") {
                reopenLastClosed()
            }
            .disabled(recentlyClosed.isEmpty)

            Divider()

            contextMenuItem("Select", systemImage: "checkmark.circle") {
                withAnimation {
                    selection.toggle(id)
                }
            }
        }
    }

    private func recentlyClosedRow(_ tab: WorkspaceTabInfo) -> some View {
        let name = tabName(tab)

        return HStack(spacing: 8) {
            Image(systemName: "line.3.horizontal")
                .font(.system(size: 12, weight: .semibold))
                .foregroundStyle(.tertiary)
                .padding(.vertical, 8)
                .padding(.trailing, 2)

            Image(systemName: "arrow.uturn.left")
                .font(.system(size: 12))
                .foregroundStyle(.secondary)

            Text(name)
                .lineLimit(1)
                .truncationMode(.tail)
                .foregroundStyle(.secondary)

            Spacer(minLength: 0)
        }
        .padding(.vertical, 6)
        .padding(.horizontal, 8)
        .contentShape(Rectangle())
        .onTapGesture {
            workspaceInput.reopenClosedTab(id: tab.id)
            withAnimation {
                refresh()
            }
        }
        .draggable(TabDragItem(id: tab.id))
    }

    private var closeCountLabel: String {
        let count = selection.selectedIds.count
        return count == 1 ? "Close 1 tab" : "Close \(count) tabs"
    }

    private func close(_ ids: [UUID]) {
        for id in ids {
            workspaceInput.closeTab(id: id)
        }
    }

    private var tabIds: [UUID] {
        tabs.map(\.id)
    }

    private func tabName(_ tab: WorkspaceTabInfo) -> String {
        if tab.destKind == .file, let fileId = tab.destFile {
            return filesModel.idsToFiles[fileId]?.name ?? tab.displayName
        }
        return tab.displayName
    }

    private func tabIcon(_ tab: WorkspaceTabInfo, name: String) -> String {
        switch tab.destKind {
        case .search: "magnifyingglass"
        case .mindMap: "point.3.connected.trianglepath.dotted"
        case .spaceInspector: "chart.pie"
        case .file: FileIconHelper.docNameToSystemImageName(name: name)
        }
    }

    private func closeAbove(_ id: UUID) {
        guard let index = tabs.firstIndex(where: { $0.id == id }) else {
            return
        }

        close(tabs[..<index].map(\.id))
    }

    private func closeBelow(_ id: UUID) {
        guard let index = tabs.firstIndex(where: { $0.id == id }) else {
            return
        }

        close(tabs[(index + 1)...].map(\.id))
    }

    private func closeSelected() {
        close(tabIds.filter { selection.selectedIds.contains($0) })

        withAnimation {
            selection.end()
        }
    }

    private func reopenLastClosed() {
        workspaceInput.reopenLastClosedTab()
        withAnimation {
            refresh()
        }
    }

    private func dropTab(_ dragged: TabDragItem?, before target: UUID?) -> Bool {
        defer {
            dropTargetId = nil
            endZoneTargeted = false
        }

        guard let dragged = dragged?.id else {
            return false
        }

        let targetIndex = target.flatMap { id in tabs.firstIndex(where: { $0.id == id }) } ?? tabs.count

        guard let from = tabs.firstIndex(where: { $0.id == dragged }) else {
            guard recentlyClosed.contains(where: { $0.id == dragged }) else {
                return false
            }

            workspaceInput.reopenClosedTab(id: dragged)
            // Reopen may insert mid-strip; move to the drop target if needed.
            let openIds = workspaceInput.getTabs().map(\.id)
            if let reopenedFrom = openIds.firstIndex(of: dragged), reopenedFrom != targetIndex {
                let to = reopenedFrom < targetIndex ? targetIndex - 1 : targetIndex
                if to != reopenedFrom, to >= 0, to < openIds.count {
                    workspaceInput.moveTab(from: reopenedFrom, to: to)
                }
            }
            withAnimation {
                refresh()
            }

            return true
        }

        let to = from < targetIndex ? targetIndex - 1 : targetIndex

        guard to != from else {
            return false
        }

        withAnimation {
            tabs.move(fromOffsets: [from], toOffset: targetIndex)
        }
        workspaceInput.moveTab(from: from, to: to)

        return true
    }

    private var supportsMultipleWindows: Bool {
        #if os(iOS)
            UIApplication.shared.supportsMultipleScenes
        #else
            true
        #endif
    }

    private func refresh() {
        tabs = workspaceInput.getTabs()
        canNavBack = workspaceInput.canNavBack()
        canNavForward = workspaceInput.canNavForward()

        let open = Set(tabs.map(\.id))
        recentlyClosed = workspaceInput.getRecentlyClosedTabs().filter { tab in
            guard !open.contains(tab.id) else { return false }
            if tab.destKind == .file {
                guard let fileId = tab.destFile else { return false }
                return filesModel.idsToFiles[fileId] != nil
            }
            return true
        }
    }
}

#Preview {
    WorkspaceTabsList(fileTreeModel: .preview)
        .environment(FilesModel.preview)
        .environment(WorkspaceInputState.preview)
        .environment(WorkspaceOutputState.preview)
}
