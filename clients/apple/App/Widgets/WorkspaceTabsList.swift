import SwiftUI
import SwiftWorkspace

struct WorkspaceTabsList: View {
    @Environment(FilesModel.self) private var filesModel
    @Environment(WorkspaceInputState.self) private var workspaceInput
    @Environment(WorkspaceOutputState.self) private var workspaceOutput

    let fileTreeModel: FileTreeModel

    @State private var tabIds: [UUID] = []
    @State private var recentlyClosed: [UUID] = []
    @State private var selection = SelectionModel()
    @State private var dropTargetId: UUID? = nil
    @State private var endZoneTargeted = false
    @State private var renameTarget: File? = nil

    var body: some View {
        ScrollView {
            LazyVStack(alignment: .leading, spacing: 2) {
                ForEach(tabIds, id: \.self) { id in
                    tabRow(id)
                }

                endDropZone

                if !recentlyClosed.isEmpty {
                    Text("Recently Closed")
                        .font(.caption)
                        .fontWeight(.medium)
                        .foregroundStyle(.secondary)
                        .padding(.top, 14)
                        .padding(.leading, 10)

                    ForEach(recentlyClosed, id: \.self) { id in
                        recentlyClosedRow(id)
                            .id("recently-closed-\(id.uuidString)")
                    }
                }
            }
            .padding(.horizontal, 8)
            .padding(.top, 4)
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
        } else {
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

    private func tabRow(_ id: UUID) -> some View {
        let isCurrent = id == workspaceOutput.openDoc
        let name = filesModel.idsToFiles[id]?.name ?? "unknown"

        return HStack(spacing: 8) {
            SelectionIndicator(selection: selection, id: id)

            Image(systemName: "line.3.horizontal")
                .font(.system(size: 12, weight: .semibold))
                .foregroundStyle(.tertiary)
                .padding(.vertical, 8)
                .padding(.trailing, 2)
                .contentShape(Rectangle())
                .draggable(id.uuidString)

            Image(systemName: FileIconHelper.docNameToSystemImageName(name: name))
                .foregroundStyle(isCurrent ? Color.accentColor : Color.secondary)

            Text(name)
                .lineLimit(1)
                .truncationMode(.tail)
                .onTapGesture {
                    selection.handleTap(id: id, orderedIds: { tabIds }) {
                        if isCurrent {
                            renameTarget = filesModel.idsToFiles[id]
                        } else {
                            workspaceInput.openFile(id: id)
                        }
                    }
                }

            Spacer(minLength: 0)

            if !selection.selecting {
                Button {
                    workspaceInput.closeDoc(id: id)
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
                workspaceInput.openFile(id: id)
            }
        }
        .selectSwipe(selection, id: id)
        .contextMenu {
            tabMenu(id)
        }
        .draggable(id.uuidString)
        .dropDestination(for: String.self) { items, _ in
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
            .dropDestination(for: String.self) { items, _ in
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
    private func tabMenu(_ id: UUID) -> some View {
        if selection.selecting {
            contextMenuItem("Done Selecting", systemImage: "checkmark.circle.fill") {
                withAnimation {
                    selection.end()
                }
            }
        } else {
            contextMenuItem("Close", systemImage: "xmark") {
                close([id])
            }

            contextMenuItem("Close Others", systemImage: "xmark.square") {
                close(tabIds.filter { $0 != id })
            }
            .disabled(tabIds.count < 2)

            contextMenuItem("Close Above", systemImage: "arrow.up.to.line") {
                closeAbove(id)
            }
            .disabled(tabIds.first == id)

            contextMenuItem("Close Below", systemImage: "arrow.down.to.line") {
                closeBelow(id)
            }
            .disabled(tabIds.last == id)

            contextMenuItem("Close All", systemImage: "xmark.circle") {
                workspaceInput.closeAllTabs()
            }

            Divider()

            contextMenuItem("Rename", systemImage: "pencil") {
                renameTarget = filesModel.idsToFiles[id]
            }

            contextMenuItem("Reopen Closed Tab", systemImage: "arrow.uturn.left") {
                reopenLastClosed()
            }
            .disabled(recentlyClosed.isEmpty)

            Divider()

            contextMenuItem("Select", systemImage: "checkmark.circle") {
                withAnimation {
                    selection.begin(with: id)
                }
            }
        }
    }

    private func recentlyClosedRow(_ id: UUID) -> some View {
        let name = filesModel.idsToFiles[id]?.name ?? "unknown"

        return HStack(spacing: 8) {
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
            workspaceInput.openFile(id: id)
        }
    }

    private var closeCountLabel: String {
        let count = selection.selectedIds.count
        return count == 1 ? "Close 1 tab" : "Close \(count) tabs"
    }

    private func close(_ ids: [UUID]) {
        for id in ids {
            workspaceInput.closeDoc(id: id)
        }
    }

    private func closeAbove(_ id: UUID) {
        guard let index = tabIds.firstIndex(of: id) else {
            return
        }

        close(Array(tabIds[..<index]))
    }

    private func closeBelow(_ id: UUID) {
        guard let index = tabIds.firstIndex(of: id) else {
            return
        }

        close(Array(tabIds[(index + 1)...]))
    }

    private func closeSelected() {
        close(tabIds.filter { selection.selectedIds.contains($0) })

        withAnimation {
            selection.end()
        }
    }

    private func reopenLastClosed() {
        guard let id = recentlyClosed.first else {
            return
        }

        workspaceInput.openFile(id: id)
    }

    private func dropTab(_ dragged: String?, before target: UUID?) -> Bool {
        defer {
            dropTargetId = nil
            endZoneTargeted = false
        }

        guard let dragged = dragged.flatMap(UUID.init),
              let from = tabIds.firstIndex(of: dragged)
        else {
            return false
        }

        let targetIndex = target.flatMap { tabIds.firstIndex(of: $0) } ?? tabIds.count
        let to = from < targetIndex ? targetIndex - 1 : targetIndex

        guard to != from else {
            return false
        }

        withAnimation {
            tabIds.move(fromOffsets: [from], toOffset: targetIndex)
        }
        workspaceInput.moveTab(from: from, to: to)

        return true
    }

    private func refresh() {
        tabIds = workspaceInput.getTabsIds()

        let open = Set(tabIds)
        recentlyClosed = workspaceInput.getRecentlyClosedTabs()
            .filter { filesModel.idsToFiles[$0] != nil && !open.contains($0) }
    }
}

#Preview {
    WorkspaceTabsList(fileTreeModel: .preview)
        .environment(FilesModel.preview)
        .environment(WorkspaceInputState.preview)
        .environment(WorkspaceOutputState.preview)
}
