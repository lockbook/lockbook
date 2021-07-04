import Foundation

public struct UsageMetrics: Codable {
    public var usages: [FileUsage]
    public var serverUsage: UsageItemMetric
    public var dataCap: UsageItemMetric
}

public struct UsageItemMetric: Codable {
    public var exact: UInt64
    public var readable: String
}

public struct FileUsage: Codable {
    public var fileId: UUID
    public var sizeBytes: UInt64
}

extension FileUsage: Identifiable {
    public var id: UUID { fileId }
}
