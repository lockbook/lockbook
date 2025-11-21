import Bridge
import Combine
import SwiftUI

public class WorkspaceOutputState: ObservableObject {

    // Should I be using combine state classes for these? Helps reduce refreshes
    @Published public var openDoc: UUID? = nil
    @Published public var selectedFolder: UUID? = nil
    @Published public var newFolderButtonPressed: () = ()
    @Published public var currentTab: WorkspaceTab = .Welcome
    @Published public var renameOpenDoc: () = ()
    @Published public var urlOpened: URL? = nil

    // Tab count includes non-files
    // Tab ids includes only file ids
    // Ideally want to combine these
    @Published public var tabCount: Int = 0
    @Published public var tabIds: [UUID] = []

    public init() {}
}

public class WorkspaceInputState: ObservableObject {
    public var coreHandle: UnsafeMutableRawPointer? = nil
    public var wsHandle: UnsafeMutableRawPointer? = nil

    public var redraw = PassthroughSubject<(), Never>()
    public var focus = PassthroughSubject<(), Never>()
    //    maybe make unfocus variable

    public init(coreHandle: UnsafeMutableRawPointer?) {
        self.coreHandle = coreHandle
    }

    public init() {}

    public func openFile(id: UUID) {
        guard let wsHandle else {
            return
        }

        let uuid = CUuid(_0: id.uuid)
        no_folder_selected(wsHandle)
        open_file(wsHandle, uuid, false)
        redraw.send(())
        //        Will crash iOS, something with caret rects. Looks rust related
        //        focus.send(())
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

    public func fileOpCompleted(fileOp: WSFileOpCompleted) {
        guard let wsHandle else { return }

        switch fileOp {
        case .Delete(let id):
            close_tab(wsHandle, id.uuidString)
        case .Rename(let id, let newName):
            tab_renamed(wsHandle, id.uuidString, newName)
        }

        redraw.send(())
    }

    // IDEALLY PROVIDED WITHIN WORKSPACE RESP
    public func getTabsIds() -> [UUID] {
        guard let wsHandle else { return [] }

        let result = get_tabs_ids(wsHandle)
        let buffer: [CUuid] = Array(
            UnsafeBufferPointer(start: result.ids, count: Int(result.size))
        )

        let newBuffer = buffer.map({ id in
            return UUID(uuid: id._0)
        })

        free_tab_ids(result)

        return newBuffer
    }
}

public enum WSFileOpCompleted {
    case Rename(id: UUID, newName: String)
    case Delete(id: UUID)
}

func createTempDir() -> URL? {
    let fileManager = FileManager.default
    let tempTempURL = URL(fileURLWithPath: NSTemporaryDirectory())
        .appendingPathComponent("editor-tmp").appendingPathComponent(
            UUID().uuidString
        )

    do {
        try fileManager.createDirectory(
            at: tempTempURL,
            withIntermediateDirectories: true,
            attributes: nil
        )
    } catch {
        return nil
    }

    return tempTempURL
}

extension WorkspaceInputState {
    public static var preview: WorkspaceInputState {
        return WorkspaceInputState()
    }
}

extension WorkspaceOutputState {
    public static var preview: WorkspaceOutputState {
        return WorkspaceOutputState()
    }
}
