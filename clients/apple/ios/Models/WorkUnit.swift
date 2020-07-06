//
//  WorkUnit.swift
//  ios
//
//  Created by Raayan Pillai on 7/6/20.
//  Copyright © 2020 Lockbook. All rights reserved.
//

import Foundation

enum WorkUnit: Decodable {
    enum CodingKeys: String, CodingKey {
        case localChange
        case serverChange
    }
    
    enum Metakeys: String, CodingKey {
        case metadata
    }
    
    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        
        if container.contains(.localChange) {
            let local = try container.nestedContainer(keyedBy: Metakeys.self, forKey: .localChange)
            let meta = try local.decode(FileMetadata.self, forKey: .metadata)
            self = .LocalChange(metadata: meta)
            return
        }
    
        let server = try container.nestedContainer(keyedBy: Metakeys.self, forKey: .serverChange)
        let meta = try server.decode(FileMetadata.self, forKey: .metadata)
        self = .ServerChange(metadata: meta)
        return
    }
    
    case LocalChange(metadata: FileMetadata)
    case ServerChange(metadata: FileMetadata)
}
