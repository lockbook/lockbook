import Bridge

public struct Account {
    public let username: String
    public let apiUrl: String
    
    public init(username: String, apiUrl: String) {
        self.username = username
        self.apiUrl = apiUrl
    }
        
    init(_ res: LbAccountRes) {
        self.username = String(cString: res.username)
        self.apiUrl = String(cString: res.api_url)
    }
}
