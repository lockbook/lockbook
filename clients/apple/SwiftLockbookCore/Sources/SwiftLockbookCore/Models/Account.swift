import Foundation

public struct Account: Codable {
    public var username: Username
    public var apiUrl: ApiUrl
    var privateKey: [UInt8]
    
    public init(username: Username, apiUrl: ApiUrl, keys: [UInt8]) {
        self.username = username
        self.apiUrl = apiUrl
        self.privateKey = keys
    }
    
    public static func fake(username: Username) -> Account {
        Account(username: username, apiUrl: "test://test.net.prod.lockbook.api", keys: [])
    }
    
    public func qualified() -> String {
        "\(username)@\(apiUrl)"
    }
    
    public typealias Username = String
    public typealias ApiUrl = String
}

extension Account: Identifiable {
    public var id: String { username }
}
