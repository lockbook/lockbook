import Foundation

public struct FileMetadata: Codable, Identifiable, Equatable {
    public var fileType: FileType
    public var id: UUID
    public var parent: UUID
    public var name: String
    public var owner: String
    public var contentVersion: UInt64
    public var metadataVersion: UInt64
    public var deleted: Bool
    public var userAccessKeys: [Account.Username : UserAccessInfo] = .init()
    public var folderAccessKeys: FolderAccessInfo = FolderAccessInfo(folderId: .init(), accessKey: .init(value: [], nonce: []))
    public var isRoot: Bool { parent == id }
    
    public static func == (lhs: FileMetadata, rhs: FileMetadata) -> Bool {
        return lhs.fileType == rhs.fileType &&
            lhs.id == rhs.id &&
            lhs.metadataVersion == rhs.metadataVersion &&
            lhs.contentVersion == rhs.contentVersion &&
            lhs.parent == rhs.parent &&
            lhs.owner == rhs.owner &&
            lhs.deleted == rhs.deleted &&
            lhs.name == rhs.name
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
    var publicKey: RSAPublicKey
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

public struct RSAPublicKey: Codable {
    var n: [UInt]
    var e: [UInt]
}
