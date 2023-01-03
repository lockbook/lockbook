import Foundation

public struct UsageMetrics: Codable {
    public var usages: [FileUsage]
    public var serverUsage: UsageItemMetric
    public var dataCap: UsageItemMetric
    
    public init(usages: [FileUsage], serverUsage: UsageItemMetric, dataCap: UsageItemMetric) {
        self.usages = usages
        self.serverUsage = serverUsage
        self.dataCap = dataCap
    }
}

public struct UsageItemMetric: Codable {
    public var exact: UInt64
    public var readable: String
    
    public init(exact: UInt64, readable: String) {
        self.exact = exact
        self.readable = readable
    }
}

public struct FileUsage: Codable {
    public var fileId: UUID
    public var sizeBytes: UInt64
    
    public init(fileId: UUID, sizeBytes: UInt64) {
        self.fileId = fileId
        self.sizeBytes = sizeBytes
    }
}

extension FileUsage: Identifiable {
    public var id: UUID { fileId }
}
