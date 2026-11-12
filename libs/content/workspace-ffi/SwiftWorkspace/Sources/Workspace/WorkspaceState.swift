import Bridge
import Combine
import Foundation
import Observation

public struct SearchRequest: Equatable {
    public let query: String
    public let content: Bool
}

@Observable public class WorkspaceOutputState {
    public var openDoc: UUID? = nil
    public var currentSession: UUID? = nil
    public var selectedFolder: UUID? = nil
    public var currentTab: WorkspaceTab = .Welcome
    public var urlsOpened: [URL] = []

    // Tab count includes non-files
    public var tabCount: Int = 0

    public var openCamera: Bool = false
    /// iOS shows search itself; a link's query fills it in.
    public var openSearch: SearchRequest? = nil
    /// Why a link didn't open, for an alert.
    public var linkMessage: String? = nil
    /// A link waits on a sync to find its file.
    public var openingLink: Bool = false
    /// The open note has headings: native chrome offers its outline.
    public var outlineAvailable: Bool = false
    /// The outline sheet is up: floating buttons would cover it.
    public var outlineOpen: Bool = false
    /// The open file has a rendered form: native chrome offers its source.
    public var sourceAvailable: Bool = false
    /// Files with a rendered form show as source.
    public var sourceOn: Bool = false

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

            let queued = pendingOpens
            pendingOpens = []
            for id in queued {
                openFile(id: id)
            }
            let links = pendingLinks
            pendingLinks = []
            for url in links {
                _ = openUrl(url)
            }
        }
    }

    @ObservationIgnored private var nativeSidebarOpen = false
    @ObservationIgnored private var pendingOpens: [UUID] = []
    @ObservationIgnored private var pendingLinks: [URL] = []
    @ObservationIgnored private var desktopTabPolicy = false

    @ObservationIgnored public var redraw = PassthroughSubject<Void, Never>()
    @ObservationIgnored public var focus = PassthroughSubject<Void, Never>()
    /// A note just created; it opens ready to type.
    @ObservationIgnored public var pendingFocusDoc: UUID?

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

    public func openFile(id: UUID, newTab: Bool = false, focus: Bool = false) {
        if focus {
            pendingFocusDoc = id
        }
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

    /// Opens a link from outside the app; false when it isn't ours. One that
    /// arrives before the workspace is up waits for it.
    public func openUrl(_ url: URL) -> Bool {
        guard let wsHandle else {
            pendingLinks.append(url)
            return true
        }

        no_folder_selected(wsHandle)
        let opened = open_url(wsHandle, url.absoluteString)
        redraw.send(())
        return opened
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

    public func toggleOutline() {
        guard let wsHandle else { return }

        toggle_outline(wsHandle)
        redraw.send(())
    }

    public func toggleSource() {
        guard let wsHandle else { return }

        toggle_source(wsHandle)
        redraw.send(())
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

    public func closeActiveTab() {
        guard let wsHandle else { return }
        close_active_tab(wsHandle)
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
    case diagram = 4
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
        case .diagram: "Diagram"
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
