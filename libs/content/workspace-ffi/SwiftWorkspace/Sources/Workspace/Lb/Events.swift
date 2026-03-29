import Bridge
import Foundation

public class Events: ObservableObject {
    @Published public var status: Status = .init()
    @Published public var pendingShares: [UUID] = []
    @Published public var metadataUpdated: Bool = false
}

public struct Status {
    public var offline: Bool
    public var syncing: Bool
    public var outOfSpace: Bool
    public var pendingShares: Bool
    public var updateRequired: Bool
    public var pushingFiles: [UUID]
    public var dirtyLocally: [UUID]
    public var pullingFiles: [UUID]
    public var spaceUsed: UsageMetrics?
    public var message: String

    init(_ status: LbStatus) {
        offline = status.offline
        syncing = status.syncing
        outOfSpace = status.out_of_space
        pendingShares = status.pending_shares
        updateRequired = status.update_required
        dirtyLocally = Array(UnsafeBufferPointer(start: status.dirty_locally.ids, count: Int(status.dirty_locally.len))).toUUIDs()
        pushingFiles = Array(UnsafeBufferPointer(start: status.pushing_files.ids, count: Int(status.pushing_files.len))).toUUIDs()
        pullingFiles = Array(UnsafeBufferPointer(start: status.pulling_files.ids, count: Int(status.pulling_files.len))).toUUIDs()
        spaceUsed = status.space_used != nil ? UsageMetrics(status.space_used.move()) : nil
        message = String(cString: status.msg)
    }

    init() {
        offline = false
        syncing = false
        outOfSpace = false
        pendingShares = false
        updateRequired = false
        pushingFiles = []
        dirtyLocally = []
        pullingFiles = []
        spaceUsed = nil
        message = ""
    }
}
