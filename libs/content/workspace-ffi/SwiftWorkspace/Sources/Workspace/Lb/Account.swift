import Bridge

public struct Account {
    public let username: String
    public let apiUrl: String

    public init(username: String, apiUrl: String) {
        self.username = username
        self.apiUrl = apiUrl
    }

    init(_ res: LbAccountRes) {
        username = String(cString: res.username)
        apiUrl = String(cString: res.api_url)
    }
}

public extension Account {
    static let subAccountSeparator: Character = "+"

    var isSubAccount: Bool {
        username.contains(Account.subAccountSeparator)
    }
}

public struct SubAccount: Identifiable, Equatable {
    public let username: String
    public let usage: UsageItem

    public var id: String {
        username
    }

    public var label: String {
        username.split(separator: Account.subAccountSeparator, maxSplits: 1).dropFirst().first.map(String.init) ?? username
    }

    public init(username: String, usage: UsageItem) {
        self.username = username
        self.usage = usage
    }

    init(_ res: LbSubAccount) {
        username = String(cString: res.username)
        usage = UsageItem(exact: res.usage)
    }
}
