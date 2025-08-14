import SwiftUI
import SwiftWorkspace

class AppState: ObservableObject {
    static let shared = AppState()

    static let LB_LOC: String = {
        #if os(macOS)
        NSHomeDirectory() + "/.lockbook"
        #else
        FileManager.default.urls(for: .documentDirectory, in: .userDomainMask).last!.path
        #endif
    }()
    static let LB_API_URL: String? = ProcessInfo.processInfo.environment["API_LOCATION"]
    
    static let lb: LbAPI = {
        if isPreviewEnvironmentKey.defaultValue {
            return MockLb()
        }
        
        return Lb(writablePath: ProcessInfo.processInfo.environment["LOCKBOOK_PATH"] ?? LB_LOC, logs: true)
    }()
    static var workspaceState: WorkspaceState = WorkspaceState()
    static var billingState: BillingState = BillingState()
    
    @Published var isLoggedIn: Bool = false
    @Published var error: UIError? = nil
    
    private init() {
        self.checkIfLoggedIn()
    }
    
    func checkIfLoggedIn() {
        switch AppState.lb.getAccount() {
        case .success(_):
            self.isLoggedIn = true
        case .failure(_):
            self.isLoggedIn = false
        }
    }
    
    static func isInternalLink(_ url: URL) -> Bool {
        return url.scheme == "lb"
    }
}

enum UIError: Identifiable {
    case lb(error: LbError)
    case custom(title: String, msg: String)
    
    var id: String {
        switch self {
        case .lb(let error): return "lb-\(error.msg)"
        case .custom(let title, _): return "custom-\(title)"
        }
    }

    var title: String {
        switch self {
        case .lb(_): return "Error"
        case .custom(let title, _): return title
        }
    }

    var message: String {
        switch self {
        case .lb(let error): return error.msg
        case .custom(_, let msg): return msg
        }
    }
}
