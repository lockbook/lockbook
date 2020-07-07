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

extension WorkUnit: Decodable {
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

struct Content: Decodable {
    var metadata: FileMetadata
}

enum WorkType: String, Decodable {
    case LocalChange
    case ServerChange
}


