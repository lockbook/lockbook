import SwiftUI
import SwiftWorkspace

class MainState: ObservableObject {
    static let shared = MainState()

    static let LB_LOC: String = {
        #if os(macOS)
        NSHomeDirectory() + "/.lockbook"
        #else
        FileManager.default.urls(for: .documentDirectory, in: .userDomainMask).last!.path
        #endif
    }()
    static let LB_API_URL: String? = ProcessInfo.processInfo.environment["API_LOCATION"]
    
    static let lb: LbAPI = {
        if ProcessInfo.processInfo.environment["XCODE_RUNNING_FOR_PREVIEWS"] == "1" {
            return MockLb()
        }
        
        return Lb(writablePath: ProcessInfo.processInfo.environment["LOCKBOOK_PATH"] ?? LB_LOC, logs: true)
    }()
    
    @Published var isLoggedIn: Bool
    
    #if os(iOS)
    @Published var platform: iOSPlatform = iOSPlatform(UIDevice.current.userInterfaceIdiom)
    #endif
    
    private init() {
        self.isLoggedIn = MainState.checkIfLoggedIn()
    }
    
    static func checkIfLoggedIn() -> Bool {
        switch MainState.lb.getAccount() {
        case .success(_):
            return true
        case .failure(_):
            return false
        }
    }
}

#if os(iOS)
enum iOSPlatform {
    case iPhone
    case iPad;
    
    init(_ idiom: UIUserInterfaceIdiom) {
        switch idiom {
        case .pad: self = .iPad
        case .phone: self = .iPhone
        default: self = .iPhone
        }
    }
}
#endif

