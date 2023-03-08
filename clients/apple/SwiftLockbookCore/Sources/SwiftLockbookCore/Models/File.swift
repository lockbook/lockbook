import Foundation

public struct File: Codable, Identifiable, Equatable, Hashable, Comparable {
    public var fileType: FileType
    public var id: UUID
    public var parent: UUID
    public var name: String
    public var lastModifiedBy: String
    public var lastModified: UInt64
    public var shares: [Share]
    
    public var isRoot: Bool { parent == id }
    public static func == (lhs: File, rhs: File) -> Bool {
        return lhs.fileType == rhs.fileType &&
            lhs.id == rhs.id &&
//            lhs.metadataVersion == rhs.metadataVersion && // TODO don't do this here, do this at the view instead
//            lhs.contentVersion == rhs.contentVersion &&
            lhs.parent == rhs.parent &&
            lhs.lastModifiedBy == rhs.lastModifiedBy &&
            lhs.name == rhs.name
    }

    public func hash(into hasher: inout Hasher) {
        hasher.combine(id)
        hasher.combine(name)
    }

    public static func <(lhs: File, rhs: File) -> Bool {
        // If the types are different, group folders higher
        if lhs.fileType == .Folder && rhs.fileType == .Document {
            return true
        }

        if rhs.fileType == .Folder && lhs.fileType == .Document {
            return false
        }

        // Otherwise sort alphabetically
        return lhs.name < rhs.name
    }
}

public enum FileType: String, Codable {
    case Document
    case Folder
}

public struct FolderAccessInfo: Codable {
    var folderId: UUID
    var accessKey: EncryptedValueWithNonce
}

public struct UserAccessInfo: Codable {
    var username: Account.Username
    var encryptedBy: String
    var accessKey: EncryptedValue
}

public struct SignedValue: Codable {
    var content: String
    var signature: String
}

public struct EncryptedValueWithNonce: Codable {
    var value: [UInt]
    var nonce: [UInt]
}

public struct EncryptedValue: Codable {
    var value: [UInt]
}

public enum ShareMode: String, Codable {
    case Write
    case Read
}

public struct Share: Codable {
    public var mode: ShareMode
    public var sharedBy: String
    public var sharedWith: String
}

public struct SearchResultItem: Identifiable, Codable {
    public var id: UUID
    public var path: String
    public var score: UInt64
}
