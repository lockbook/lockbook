import Foundation

public enum ConfigHelper {
    enum Keys: String {
        case apiLocation = "API_LOCATION"
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
}
