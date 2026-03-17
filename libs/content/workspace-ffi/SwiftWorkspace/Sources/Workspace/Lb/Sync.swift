import Foundation
import Bridge


public struct UsageMetrics {
    public let serverUsedExact: UInt64
    public let serverUsedHuman: String
    
    public let serverCapExact: UInt64
    public let serverCapHuman: String
    
    public init(serverUsedExact: UInt64, serverUsedHuman: String, serverCapExact: UInt64, serverCapHuman: String) {
        self.serverUsedExact = serverUsedExact
        self.serverUsedHuman = serverUsedHuman
        self.serverCapExact = serverCapExact
        self.serverCapHuman = serverCapHuman
    }
    
    init(_ res: LbUsageMetrics) {
        self.serverUsedExact = res.server_used_exact
        self.serverUsedHuman = String(cString: res.server_used_human)
        self.serverCapExact = res.server_cap_exact
        self.serverCapHuman = String(cString: res.server_cap_human)
    }
}
