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
    
    @Published var isLoggedIn: Bool = false
    
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
}
