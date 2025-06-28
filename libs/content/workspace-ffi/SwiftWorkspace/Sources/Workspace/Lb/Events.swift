import Bridge
import Foundation

public class Events: ObservableObject {
    @Published public var status: Status = Status()
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
    public var syncStatus: String
    
    init(_ status: LbStatus) {
        self.offline = status.offline
        self.syncing = status.syncing
        self.outOfSpace = status.out_of_space
        self.pendingShares = status.pending_shares
        self.updateRequired = status.update_required
        self.dirtyLocally = Array(UnsafeBufferPointer(start: status.dirty_locally.ids, count: Int(status.dirty_locally.len))).toUUIDs()
        self.pushingFiles = Array(UnsafeBufferPointer(start: status.pushing_files.ids, count: Int(status.pushing_files.len))).toUUIDs()
        self.pullingFiles = Array(UnsafeBufferPointer(start: status.pulling_files.ids, count: Int(status.pulling_files.len))).toUUIDs()
        self.spaceUsed = status.space_used != nil ? UsageMetrics(status.space_used.move()) : nil
        self.syncStatus = status.sync_status != nil ? String(cString: status.sync_status) : ""
    }

    init() {
        self.offline = false
        self.syncing = false
        self.outOfSpace = false
        self.pendingShares = false
        self.updateRequired = false
        self.pushingFiles = []
        self.dirtyLocally = []
        self.pullingFiles = []
        self.spaceUsed = nil
        self.syncStatus = ""
    }
}

