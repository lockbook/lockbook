//
//  FileMetadata.swift
//  ios_client
//
//  Created by Raayan Pillai on 4/11/20.
//  Copyright © 2020 Lockbook. All rights reserved.
//

import Foundation

struct FileMetadata: Codable, Identifiable {
    var fileType: FileType
    var id: UUID
    var parent: UUID
    var name: String
    var owner: String
    var contentVersion: Int
    var metadataVersion: Int
    var deleted: Bool
    var signature: SignedValue = SignedValue(content: "", signature: "")
    var userAccessKeys: [Account.Username : UserAccessInfo] = .init()
    var folderAccessKeys: FolderAccessInfo = FolderAccessInfo(folderId: .init(), accessKey: .init(garbage: "", nonce: ""))
}

enum FileType: String, Codable {
    case Document
    case Folder
}

struct FolderAccessInfo: Codable {
    var folderId: UUID
    var accessKey: EncryptedValueWithNonce
}

struct UserAccessInfo: Codable {
    var username: Account.Username
    var publicKey: RSAPublicKey
    var accessKey: EncryptedValue
}

struct SignedValue: Codable {
    var content: String
    var signature: String
}

struct EncryptedValueWithNonce: Codable {
    var garbage: String
    var nonce: String
}

struct EncryptedValue: Codable {
    var garbage: String
}

struct RSAPublicKey: Codable {
    var n: [UInt]
    var e: [UInt]
}
