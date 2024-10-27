import Bridge

public struct Account {
    let username: String
    let apiUrl: String
        
    init(_ res: LbAccountRes) {
        self.username = String(cString: res.username)
        self.apiUrl = String(cString: res.api_url)
    }
}
