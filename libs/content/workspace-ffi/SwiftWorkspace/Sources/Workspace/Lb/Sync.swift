import Foundation
import Bridge

public struct SyncStatus {
    public let latestServerTS: UInt64
    public let work: [WorkUnit]
    
    public init(latestServerTS: UInt64, work: [WorkUnit]) {
        self.latestServerTS = latestServerTS
        self.work = work
    }
    
    init(_ res: LbSyncRes) {
        self.latestServerTS = res.latest_server_ts
        self.work = Array(UnsafeBufferPointer(start: res.work.work, count: Int(res.work.len))).toWorkUnits()
    }
}

extension Array<LbWorkUnit> {
    func toWorkUnits() -> [WorkUnit] {
        var workUnits: [WorkUnit] = []
        
        for workUnit in self {
            workUnits.append(WorkUnit(workUnit))
        }
        
        return workUnits
    }
}

public struct WorkUnit {
    public let id: UUID
    public let local: Bool
    
    init(_ workUnit: LbWorkUnit) {
        self.id = workUnit.id.toUUID()
        self.local = workUnit.local
    }
}

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

public struct UncompressedUsageMetric {
    public let exact: UInt64
    public let humanMsg: String
    
    public init (exact: UInt64, humanMsg: String) {
        self.exact = exact
        self.humanMsg = humanMsg
    }
    
    init(_ res: LbUncompressedRes) {
        self.exact = res.uncompressed_exact
        self.humanMsg = String(cString: res.uncompressed_human)
    }
}
