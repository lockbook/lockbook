import Foundation

public struct WorkUnit: Decodable {
    public var content: UUID
    public var tag: String
}

public struct SyncStatus: Decodable {
    public var latestServerTs: UInt64
    public var workUnits: [WorkUnit]
}

