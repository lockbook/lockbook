import Foundation

public struct FileMetadata: Codable, Identifiable {
    public var fileType: FileType
    public var id: UUID
    public var parent: UUID
    public var name: String
    public var owner: String
    public var contentVersion: UInt64
    public var metadataVersion: UInt64
    public var deleted: Bool
}

public enum FileType: String, Codable {
    case Document
    case Folder
}
