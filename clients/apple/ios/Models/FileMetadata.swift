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
}

enum FileType: String, Codable {
    case Document
    case Folder
}

enum Status: String, Codable {
    case New
    case Local
    case Remote
    case Synced
}
