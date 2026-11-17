import Bridge
import Foundation

public struct UsageMetrics: Equatable {
    public let synced: UsageItem
    public let local: UsageItem
    public let added: UsageItem
    public let freed: UsageItem
    public let dataCap: UsageItem?

    public var overCap: Bool {
        dataCap.map { local.exact > $0.exact } ?? false
    }

    public init(synced: UsageItem, local: UsageItem, added: UsageItem, freed: UsageItem, dataCap: UsageItem?) {
        self.synced = synced
        self.local = local
        self.added = added
        self.freed = freed
        self.dataCap = dataCap
    }

    init(_ res: LbUsageMetrics) {
        synced = UsageItem(res.synced)
        local = UsageItem(res.local)
        added = UsageItem(res.added)
        freed = UsageItem(res.freed)
        dataCap = res.has_data_cap ? UsageItem(res.data_cap) : nil
    }

    public static let empty = UsageMetrics(synced: .zero, local: .zero, added: .zero, freed: .zero, dataCap: nil)
}

public struct UsageItem: Equatable {
    public let exact: UInt64
    public let human: String

    public init(exact: UInt64, human: String) {
        self.exact = exact
        self.human = human
    }

    public init(exact: UInt64) {
        self.exact = exact
        let human = lb_bytes_to_human(exact)
        self.human = String(cString: human!)
        lb_free_str(human)
    }

    init(_ res: LbUsageItem) {
        exact = res.exact
        human = res.human != nil ? String(cString: res.human) : ""
    }

    public static let zero = UsageItem(exact: 0, human: "0 B")
}

public struct UsageEntry: Equatable {
    public let id: UUID
    public let synced: UInt64
    public let local: UInt64

    public init(id: UUID, synced: UInt64, local: UInt64) {
        self.id = id
        self.synced = synced
        self.local = local
    }
}

