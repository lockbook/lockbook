import Foundation

public struct FileUsage: Codable {
    public var fileId: UUID
    public var byteSecs: UInt64
    public var secs: UInt64
}
