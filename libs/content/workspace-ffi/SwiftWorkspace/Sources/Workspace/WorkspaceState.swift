import Bridge
import Combine
import Foundation
import Observation

@Observable public class WorkspaceOutputState {
    public var openDoc: UUID? = nil
    public var currentSession: UUID? = nil
    public var selectedFolder: UUID? = nil
    public var currentTab: WorkspaceTab = .Welcome
    public var urlsOpened: [URL] = []

    // Tab count includes non-files
    public var tabCount: Int = 0

    public var openCamera: Bool = false

    public var mobileToolbarShown: Bool = false

    public init() {}
}

@Observable public class WorkspaceInputState {
    @ObservationIgnored public var coreHandle: UnsafeMutableRawPointer?
    @ObservationIgnored public var wsHandle: UnsafeMutableRawPointer? {
        didSet {
            guard let wsHandle else { return }
            set_sidebar_open(wsHandle, nativeSidebarOpen)
            set_desktop_tab_policy(wsHandle, desktopTabPolicy)

            guard !pendingOpens.isEmpty else { return }
            let queued = pendingOpens
            pendingOpens = []
            for id in queued {
                openFile(id: id)
            }
        }
    }

    @ObservationIgnored private var nativeSidebarOpen = false
    @ObservationIgnored private var pendingOpens: [UUID] = []
    @ObservationIgnored private var desktopTabPolicy = false

    @ObservationIgnored public var redraw = PassthroughSubject<Void, Never>()
    @ObservationIgnored public var focus = PassthroughSubject<Void, Never>()

    public init(coreHandle: UnsafeMutableRawPointer?) {
        self.coreHandle = coreHandle
    }

    public init() {}

    #if os(iOS)
        deinit {
            guard let wsHandle else { return }
            DispatchQueue.main.async {
                WorkspaceControllerRegistry.controllers.removeValue(forKey: wsHandle)
            }
        }
    #endif

    public func activateTab(id: UUID) {
        guard let wsHandle else { return }

        activate_tab(wsHandle, CUuid(_0: id.uuid))
        redraw.send(())
    }

    public func setDesktopTabPolicy(_ desktop: Bool) {
        desktopTabPolicy = desktop
        guard let wsHandle else { return }
        set_desktop_tab_policy(wsHandle, desktop)
        redraw.send(())
    }

    public func openFile(id: UUID, newTab: Bool = false) {
        guard let wsHandle else {
            pendingOpens.append(id)
            return
        }

        let uuid = CUuid(_0: id.uuid)
        no_folder_selected(wsHandle)
        open_file(wsHandle, uuid, newTab)
        redraw.send(())
        //        Will crash iOS, something with caret rects. Looks rust related
        //        focus.send(())
    }

    public func navigateToFile(id: UUID) {
        guard let wsHandle else { return }

        navigate_to_file(wsHandle, CUuid(_0: id.uuid))
        redraw.send(())
    }

    public func navigateToFile(id: UUID, rangeStart: Int, rangeEnd: Int) {
        guard let wsHandle else { return }

        navigate_to_file_at(wsHandle, CUuid(_0: id.uuid), UInt(rangeStart), UInt(rangeEnd))
        redraw.send(())
    }

    public func openFile(id: UUID, rangeStart: Int, rangeEnd: Int, newTab: Bool = false) {
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

    public func navBack() {
        guard let wsHandle else { return }

        nav_back(wsHandle)
        redraw.send(())
    }

    public func navForward() {
        guard let wsHandle else { return }

        nav_forward(wsHandle)
        redraw.send(())
    }

    public func canNavBack() -> Bool {
        guard let wsHandle else { return false }

        return can_nav_back(wsHandle)
    }

    public func canNavForward() -> Bool {
        guard let wsHandle else { return false }

        return can_nav_forward(wsHandle)
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

        redraw.send(())
        DispatchQueue.global(qos: .userInitiated).async {
            request_sync(wsHandle)
        }
    }

    public func closeDoc(id: UUID) {
        guard let wsHandle else { return }

        close_tab(wsHandle, id.uuidString)
        redraw.send(())
    }

    public func closeTab(id: UUID) {
        guard let wsHandle else { return }

        close_session(wsHandle, CUuid(_0: id.uuid))
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

    public func getTabs() -> [WorkspaceTabInfo] {
        guard let wsHandle else { return [] }

        return tabInfos(from: get_tabs(wsHandle))
    }

    public func getRecentlyClosedTabs() -> [WorkspaceTabInfo] {
        guard let wsHandle else { return [] }

        return tabInfos(from: get_recently_closed_tabs(wsHandle))
    }

    public func reopenLastClosedTab() {
        guard let wsHandle else { return }

        reopen_last_closed_tab(wsHandle)
        redraw.send(())
    }

    public func reopenClosedTab(id: UUID) {
        guard let wsHandle else { return }

        reopen_closed_tab(wsHandle, CUuid(_0: id.uuid))
        redraw.send(())
    }

    public func moveTab(from: Int, to: Int) {
        guard let wsHandle else { return }

        reorder_tab(wsHandle, UInt(from), UInt(to))
        redraw.send(())
    }

    public func setSidebarOpen(_ open: Bool) {
        nativeSidebarOpen = open

        if let wsHandle {
            set_sidebar_open(wsHandle, open)
            redraw.send(())
        }
    }

    private func tabInfos(from result: CTabs) -> [WorkspaceTabInfo] {
        let buffer: [CTabInfo] = Array(
            UnsafeBufferPointer(start: result.tabs, count: Int(result.size))
        )

        let infos = buffer.map(WorkspaceTabInfo.init)
        free_tabs(result)
        return infos
    }
}

public enum WorkspaceDestKind: Int32 {
    case file = 0
    case search = 1
    case mindMap = 2
    case spaceInspector = 3
}

public struct WorkspaceTabInfo: Identifiable, Hashable {
    public let id: UUID
    public let destKind: WorkspaceDestKind
    public let destFile: UUID?

    init(_ info: CTabInfo) {
        id = UUID(uuid: info.session_id._0)
        destKind = WorkspaceDestKind(rawValue: info.dest_kind) ?? .file
        let file = UUID(uuid: info.dest_file._0)
        destFile = file.isNil() ? nil : file
    }

    public var displayName: String {
        switch destKind {
        case .search: "Search"
        case .mindMap: "Mind Map"
        case .spaceInspector: "Space Inspector"
        case .file: "unknown"
        }
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
