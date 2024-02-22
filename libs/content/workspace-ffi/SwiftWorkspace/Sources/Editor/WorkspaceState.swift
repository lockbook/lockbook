import SwiftUI
import Combine
import Bridge

// todo can this go away enirely?
public class WorkspaceState: ObservableObject {

    @Published public var pasted: Bool = false
    @Published public var shouldFocus: Bool = false
    
    @Published public var openDoc: UUID? = nil {
        didSet {
            #if os(macOS)
            shouldFocus = true
            #endif
            pendingSharesOpen = false
        }
    }
    
    @Published public var pendingSharesOpen: Bool = false
    
    @Published public var selectedFolder: UUID? = nil
    
    @Published public var syncing: Bool = false
    @Published public var clientUpgrade: Bool = false
    @Published public var outOfSpace: Bool = false
    @Published public var offline: Bool = false
    @Published public var syncProgress: Float? = nil
    @Published public var statusMsg: String = ""
    
    @Published public var reloadFiles: Bool = false
    @Published public var syncRequested: Bool = false
    
    @Published public var newFolderButtonPressed: Bool = false
    
    @Published public var currentTab: WorkspaceTab = .Welcome
    
    @Published public var renameOpenDoc: Bool = false
    @Published public var renameCompleted: WSRenameCompleted? = nil
    @Published public var closeActiveTab: Bool = false
    
    #if os(iOS)
    @Published public var dragOffset: CGFloat = 0.0
    #endif
    
    public init() {}
    
    public func requestSync() {
        self.syncRequested = true
    }
}

public struct WSRenameCompleted {
    public init(id: UUID, newName: String) {
        self.id = id
        self.newName = newName
    }
    
    public let id: UUID
    public let newName: String
}

func createTempDir() -> URL? {
    let fileManager = FileManager.default
    let tempTempURL = URL(fileURLWithPath: NSTemporaryDirectory()).appendingPathComponent("editor-tmp").appendingPathComponent(UUID().uuidString)
    
    do {
        try fileManager.createDirectory(at: tempTempURL, withIntermediateDirectories: true, attributes: nil)
    } catch {
        return nil
    }
    
    return tempTempURL
}
