import Bridge
import Combine
import Foundation
import Observation

@Observable public class WorkspaceOutputState {
    public var openDoc: UUID? = nil
    public var selectedFolder: UUID? = nil
    public var currentTab: WorkspaceTab = .Welcome
    public var urlsOpened: [URL] = []

    // Tab count includes non-files
    public var tabCount: Int = 0

    public var openCamera: Bool = false

    public init() {}
}

@Observable public class WorkspaceInputState {
    @ObservationIgnored public var coreHandle: UnsafeMutableRawPointer?
    @ObservationIgnored public var wsHandle: UnsafeMutableRawPointer?

    @ObservationIgnored public var redraw = PassthroughSubject<Void, Never>()
    @ObservationIgnored public var focus = PassthroughSubject<Void, Never>()

    public init(coreHandle: UnsafeMutableRawPointer?) {
        self.coreHandle = coreHandle
    }

    public init() {}

    public func openFile(id: UUID, newTab: Bool = true) {
        guard let wsHandle else {
            return
        }

        let uuid = CUuid(_0: id.uuid)
        no_folder_selected(wsHandle)
        open_file(wsHandle, uuid, newTab)
        redraw.send(())
        //        Will crash iOS, something with caret rects. Looks rust related
        //        focus.send(())
    }

    public func openFile(id: UUID, rangeStart: Int, rangeEnd: Int, newTab: Bool = true) {
        guard let wsHandle else { return }

        let uuid = CUuid(_0: id.uuid)
        no_folder_selected(wsHandle)
        open_file_at(wsHandle, uuid, UInt(rangeStart), UInt(rangeEnd), newTab)
        redraw.send(())
    }

    public func selectFolder(id: UUID?) {
        guard let wsHandle else { return }

        if let id {
            folder_selected(wsHandle, CUuid(_0: id.uuid))
        } else {
            no_folder_selected(wsHandle)
        }
        redraw.send(())
    }

    public func showSearch() {
        guard let wsHandle else { return }

        show_search(wsHandle)
        redraw.send(())
        focus.send(())
    }

    public func showFindInDoc() {
        guard let wsHandle else { return }

        show_find(wsHandle)
        redraw.send(())
    }

    public func createDocAt(parent: UUID, drawing: Bool) {
        guard let wsHandle else { return }

        let parent = CUuid(_0: parent.uuid)
        create_doc_at(wsHandle, parent, drawing)
        redraw.send(())
    }

    public func requestSync() {
        guard let wsHandle else { return }

        request_sync(wsHandle)
        redraw.send(())
    }

    public func closeDoc(id: UUID) {
        guard let wsHandle else { return }

        close_tab(wsHandle, id.uuidString)
        redraw.send(())
    }

    public func closeAllTabs() {
        guard let wsHandle else { return }

        close_all_tabs(wsHandle)
        redraw.send(())
    }

    public func pasteImage(data: Data, isPaste: Bool) {
        guard let wsHandle else { return }

        let imgPtr = data.withUnsafeBytes {
            (pointer: UnsafeRawBufferPointer) -> UnsafePointer<UInt8> in
            return pointer.baseAddress!.assumingMemoryBound(to: UInt8.self)
        }

        clipboard_send_image(wsHandle, imgPtr, UInt(data.count), isPaste)
        redraw.send(())
    }

    public func fileOpCompleted(fileOp: WSFileOpCompleted) {
        guard let wsHandle else { return }

        switch fileOp {
        case let .Delete(id):
            close_tab(wsHandle, id.uuidString)
        case let .Rename(id, newName):
            tab_renamed(wsHandle, id.uuidString, newName)
        }

        redraw.send(())
    }

    /// IDEALLY PROVIDED WITHIN WORKSPACE RESP
    public func getTabsIds() -> [UUID] {
        guard let wsHandle else { return [] }

        return tabIds(from: get_tabs_ids(wsHandle))
    }

    public func getRecentlyClosedTabs() -> [UUID] {
        guard let wsHandle else { return [] }

        return tabIds(from: get_recently_closed_tabs(wsHandle))
    }

    public func moveTab(from: Int, to: Int) {
        guard let wsHandle else { return }

        reorder_tab(wsHandle, UInt(from), UInt(to))
        redraw.send(())
    }

    private func tabIds(from result: TabsIds) -> [UUID] {
        let buffer: [CUuid] = Array(
            UnsafeBufferPointer(start: result.ids, count: Int(result.size))
        )

        let newBuffer = buffer.map { id in
            UUID(uuid: id._0)
        }

        free_tab_ids(result)

        return newBuffer
    }
}

public enum WSFileOpCompleted {
    case Rename(id: UUID, newName: String)
    case Delete(id: UUID)
}

public extension WorkspaceInputState {
    static var preview: WorkspaceInputState {
        WorkspaceInputState()
    }
}

public extension WorkspaceOutputState {
    static var preview: WorkspaceOutputState {
        WorkspaceOutputState()
    }
}
