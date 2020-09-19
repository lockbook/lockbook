import Foundation

public struct Account: Codable {
    public typealias Username = String
    
    public init(username: Username) {
        self.username = username
    }
    
    public var username: Username
    // var keys: String
}

extension Account: Identifiable {
    public var id: String { username }
}
