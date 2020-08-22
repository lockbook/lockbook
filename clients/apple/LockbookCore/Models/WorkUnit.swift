//
//  WorkUnit.swift
//  ios
//
//  Created by Raayan Pillai on 7/6/20.
//  Copyright © 2020 Lockbook. All rights reserved.
//

import Foundation

enum WorkUnit {
    case Local(Content)
    case Server(Content)
}

extension WorkUnit: Codable {
    func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        
        switch self {
        case .Local(let content):
            try container.encode(WorkType.LocalChange, forKey: CodingKeys.tag)
            try container.encode(content, forKey: CodingKeys.content)
        case .Server(let content):
            try container.encode(WorkType.LocalChange, forKey: CodingKeys.tag)
            try container.encode(content, forKey: CodingKeys.content)
        }
    }
    
    private enum CodingKeys: CodingKey {
        case content
        case tag
    }
    
    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let tag = try container.decode(WorkType.self, forKey: .tag)
        let content = try container.decode(Content.self, forKey: .content)
        
        switch tag {
        case .LocalChange:
            self = WorkUnit.Local(content)
        case .ServerChange:
            self = WorkUnit.Server(content)
        }
    }
}

struct WorkMetadata: Decodable {
    var mostRecentUpdateFromServer: Date
    var workUnits: [WorkUnit]
}

struct Content: Codable {
    var metadata: FileMetadata
}

enum WorkType: String, Codable {
    case LocalChange
    case ServerChange
}


