//
//  FileMetadata.swift
//  ios_client
//
//  Created by Raayan Pillai on 4/11/20.
//  Copyright © 2020 Lockbook. All rights reserved.
//

import Foundation

struct FileMetadata: Codable, Identifiable {
    var id: String {
        return fileId
    }
    var fileId: String
    var fileName: String
    var filePath: String
    var fileContentVersion: Int
    var fileMetadataVersion: Int
    var newFile: Bool
    var contentEditedLocally: Bool
    var metadataEditedLocally: Bool
    var deletedLocally: Bool
}

enum Status: String, Codable {
    case New
    case Local
    case Remote
    case Synced
}
