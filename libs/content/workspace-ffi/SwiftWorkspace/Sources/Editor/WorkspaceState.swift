import SwiftUI
import Combine

// todo can this go away enirely?
public class WorkspaceState: ObservableObject {

    @Published public var reloadView: Bool = false
    @Published public var pasted: Bool = false
    @Published public var shouldFocus: Bool
    
    @Published public var openDoc: UUID? = nil
    @Published public var selectedFolder: UUID? = nil
    
    public var isiPhone: Bool
    
//    public var importFile: (URL) -> String?
    
    public init() {
        self.isiPhone = false // todo smail
//        self.importFile = importFile
        
        self.shouldFocus = !isiPhone
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

