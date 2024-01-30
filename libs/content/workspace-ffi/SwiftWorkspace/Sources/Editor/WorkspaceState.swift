import SwiftUI
import Combine
#if os(iOS)
import GameController
#endif


// todo can this go away enirely?
public class WorkspaceState: ObservableObject {

    @Published public var pasted: Bool = false
    @Published public var shouldFocus: Bool
    
    @Published public var openDoc: UUID? = nil {
        didSet {
            #if os(iOS)
            shouldFocus = GCKeyboard.coalesced != nil
            #else
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
    @Published public var renameCompleted: String? = nil
    @Published public var closeActiveTab: Bool = false
        
    public var importFile: (URL) -> String?
    
    public init(importFile: @escaping (URL) -> String?) {
        self.importFile = importFile
        self.shouldFocus = false
    }
    
    public func requestSync() {
        self.syncRequested = true
    }
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

