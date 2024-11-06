import Bridge

public struct Account {
    public let username: String
    public let apiUrl: String
        
    init(_ res: LbAccountRes) {
        self.username = String(cString: res.username)
        self.apiUrl = String(cString: res.api_url)
    }
}
