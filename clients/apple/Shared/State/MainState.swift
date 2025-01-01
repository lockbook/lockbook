import SwiftUI
import SwiftWorkspace

class MainState: ObservableObject {
    static let shared = MainState()

    #if os(macOS)
    static let location: String =  NSHomeDirectory() + "/.lockbook"
    #else
    static let location: String =  FileManager.default.urls(for: .documentDirectory, in: .userDomainMask).last!.path
    #endif
    static let lb = Lb(writablePath: ProcessInfo.processInfo.environment["LOCKBOOK_PATH"] ?? location, logs: true)
    
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

