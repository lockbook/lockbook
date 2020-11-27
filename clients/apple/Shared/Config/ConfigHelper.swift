import Foundation

public enum ConfigHelper {
    enum Keys: String {
        case apiLocation = "API_LOCATION"
        case lockbookLocation = "LOCKBOOK_LOCATION"
    }
    private static let infoDictionary: [String: Any] = {
        guard let dict = Bundle.main.infoDictionary else {
            fatalError("Plist file not found")
        }
        return dict
    }()
    
    static func safeGet<T>(_ key: Keys) -> T? {
        infoDictionary[key.rawValue] as? T
    }
    
    static func get<T>(_ key: Keys) -> T {
        safeGet(key)!
    }

    static func getEnv(_ key: Keys) -> String? {
        ProcessInfo.processInfo.environment[key.rawValue]
    }

    #if os(macOS)
    static let location: String =  FileManager.default.homeDirectoryForCurrentUser.path + "/.lockbook"
    #else
    static let location: String =  FileManager.default.urls(for: .documentDirectory, in: .userDomainMask).last!.path
    #endif
}
