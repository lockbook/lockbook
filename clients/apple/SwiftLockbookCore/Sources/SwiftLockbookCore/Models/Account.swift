import Foundation

public struct Account: Codable {
    public var username: Username
    public var apiUrl: ApiUrl
    var privateKey: RSAPrivateKey
    
    public init(username: Username, apiUrl: ApiUrl, privateKey: RSAPrivateKey) {
        self.username = username
        self.apiUrl = apiUrl
        self.privateKey = privateKey
    }
    
    public static func fake(username: Username) -> Account {
        Account(username: username, apiUrl: "test://api", privateKey: .empty)
    }
    
    public func qualified() -> String {
        "\(username)@\(apiUrl)"
    }
    
    public typealias Username = String
    public typealias ApiUrl = String
    public struct RSAPrivateKey: Codable {
        var n: [UInt64]
        var e: [UInt64]
        var d: [UInt64]
        var primes: [[UInt64]]
        
        static let empty: RSAPrivateKey = RSAPrivateKey(n: [], e: [], d: [], primes: [])
    }
}

extension Account: Identifiable {
    public var id: String { username }
}
