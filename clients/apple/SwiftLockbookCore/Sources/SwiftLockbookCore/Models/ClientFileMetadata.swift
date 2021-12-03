import Foundation

public struct DecryptedFileMetadata: Codable, Identifiable, Equatable, Hashable {
    
    public var fileType: FileType
    public var id: UUID
    public var parent: UUID
    public var decryptedName: String
    public var owner: String
    public var contentVersion: UInt64
    public var metadataVersion: UInt64
    public var isRoot: Bool { parent == id }
    
    public init(fileType: FileType, id: UUID, parent: UUID, decryptedName: String, owner: String, contentVersion: UInt64, metadataVersion: UInt64) {
        self.fileType = fileType
        self.id = id
        self.parent = parent
        self.decryptedName = decryptedName
        self.owner = owner
        self.contentVersion = contentVersion
        self.metadataVersion = metadataVersion
    }
    
    public static func == (lhs: DecryptedFileMetadata, rhs: DecryptedFileMetadata) -> Bool {
        return lhs.fileType == rhs.fileType &&
            lhs.id == rhs.id &&
//            lhs.metadataVersion == rhs.metadataVersion && // TODO don't do this here, do this at the view instead
//            lhs.contentVersion == rhs.contentVersion &&
            lhs.parent == rhs.parent &&
            lhs.owner == rhs.owner &&
            lhs.decryptedName == rhs.decryptedName
    }

    public func hash(into hasher: inout Hasher) {
        hasher.combine(id)
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
