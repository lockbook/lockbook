import Foundation

public enum WorkUnit {
    case Local(Content)
    case Server(Content)
    
    public func type() -> String {
        switch self {
        case .Local(_):
            return "Local"
        case .Server(_):
            return "Server"
        }
    }
    public func get() -> FileMetadata {
        switch self {
        case .Local(let c):
            return c.metadata
        case .Server(let c):
            return c.metadata
        }
    }
}

extension WorkUnit: Codable {
    public func encode(to encoder: Encoder) throws {
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
    
    public init(from decoder: Decoder) throws {
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

public struct WorkMetadata: Decodable {
    public var mostRecentUpdateFromServer: Date
    public var workUnits: [WorkUnit]
}

public struct Content: Codable {
    public var metadata: FileMetadata
}

public enum WorkType: String, Codable {
    case LocalChange
    case ServerChange
}


