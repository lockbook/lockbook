import Foundation
import Bridge

public struct SyncStatus {
    let latestServerTS: UInt64
    let work: [WorkUnit]
    
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
    let id: UUID
    let local: Bool
    
    init(_ workUnit: LbWorkUnit) {
        self.id = workUnit.id.toUUID()
        self.local = workUnit.local
    }
}

public struct UsageMetrics {
    let serverUsedExact: UInt64
    let serverUsedHuman: String
    
    let serverCapExact: UInt64
    let serverCapHuman: String
    
    init(_ res: LbUsageMetrics) {
        self.serverUsedExact = res.server_used_exact
        self.serverUsedHuman = String(cString: res.server_used_human)
        self.serverCapExact = res.server_cap_exact
        self.serverCapHuman = String(cString: res.server_cap_human)
    }
}

public struct UncompressedUsageMetric {
    let uncompressedExact: UInt64
    let uncompressedHuman: String
    
    init(_ res: LbUncompressedRes) {
        self.uncompressedExact = res.uncompressed_exact
        self.uncompressedHuman = String(cString: res.uncompressed_human)
    }
}
