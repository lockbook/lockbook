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
    public var signature: SignedValue = SignedValue(content: "", signature: "")
    public var userAccessKeys: [Account.Username : UserAccessInfo] = .init()
    public var folderAccessKeys: FolderAccessInfo = FolderAccessInfo(folderId: .init(), accessKey: .init(garbage: "", nonce: ""))
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
    var garbage: String
    var nonce: String
}

public struct EncryptedValue: Codable {
    var garbage: String
}

public struct RSAPublicKey: Codable {
    var n: [UInt]
    var e: [UInt]
}
