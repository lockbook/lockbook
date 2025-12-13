import Bridge
import Foundation

public struct File: Codable, Identifiable, Equatable, Hashable, Comparable {
    public var id: UUID
    public var parent: UUID
    public var name: String
    public var type: FileType
    public var lastModifiedBy: String
    public var lastModified: UInt64
    public var shares: [Share]
        
    init(id: UUID, parent: UUID, name: String, type: FileType, lastModifiedBy: String, lastModified: UInt64, shares: [Share]) {
        self.id = id
        self.parent = parent
        self.name = name
        self.type = type
        self.lastModifiedBy = lastModifiedBy
        self.lastModified = lastModified
        self.shares = shares
    }
    
    init(_ file: LbFile) {
        self.id = file.id.toUUID()
        self.parent = file.parent.toUUID()
        self.name = String(cString: file.name)
        self.type = FileType(file.typ)
        self.lastModifiedBy = String(cString: file.lastmod_by)
        self.lastModified = file.lastmod
        self.shares = Array(UnsafeBufferPointer(start: file.shares.list, count: Int(file.shares.count))).toShares()
    }
    
    public var isRoot: Bool { parent == id }
    public var isFolder: Bool { self.type == .folder }
        
    public static func == (lhs: File, rhs: File) -> Bool {
        return lhs.type == rhs.type &&
            lhs.id == rhs.id &&
            lhs.shares == rhs.shares &&
            lhs.parent == rhs.parent &&
            lhs.lastModifiedBy == rhs.lastModifiedBy &&
            lhs.name == rhs.name
    }
    
    public func hash(into hasher: inout Hasher) {
        hasher.combine(id)
        hasher.combine(name)
        hasher.combine(parent)
    }
    
    public static func <(lhs: File, rhs: File) -> Bool {
        if lhs.type == .folder && rhs.type == .document {
            return true
        }

        if rhs.type == .folder && lhs.type == .document {
            return false
        }

        return lhs.name < rhs.name
    }
    
    public func shareFrom(to: String) -> String? {
        return self.shares.filter({ $0.with == to }).first?.by
    }
}

extension Array<LbFile> {
    func toFiles() -> [File] {
        var files: [File] = []
        
        for file in self {
            files.append(File(file))
        }
        
        return files
    }
}

public struct Share: Codable, Equatable {
    public let by: String
    public let with: String
    public let mode: ShareMode
    
    init(by: String, with: String, mode: ShareMode) {
        self.by = by
        self.with = with
        self.mode = mode
    }
}

public enum ShareMode: Int, Codable, Hashable {
    case write = 0
    case read = 1
    
    func toLbShareMode() -> Bridge.ShareMode {
        Bridge.ShareMode(UInt32(self.rawValue))
    }
}

extension Array<LbShare> {
    func toShares() -> [Share] {
        var shares: [Share] = []
        
        for share in self {
            shares.append(Share(by: String(cString: share.by), with: String(cString: share.with), mode: ShareMode(rawValue: Int(share.mode.rawValue))!))
        }
        
        return shares
    }
}

public enum FileType: Codable, Equatable {
    case document
    case folder
    case link(UUID)

    var rawValue: Int {
        get {
            switch self {
            case .document: return 0
            case .folder: return 1
            case .link(_): return 2
            }
        }
    }
    
    var lbLinkTarget: LbUuid {
        get {
            switch self {
            case .document: return LbUuid(bytes: (0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0 ))
            case .folder: return LbUuid(bytes: (0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0 ))
            case .link(let id): return id.toLbUuid()
            }
        }
    }
    
    init(_ fileType: LbFileType) {
        switch fileType.tag.rawValue {
        case LbDocument.rawValue: self = .document
        case LbFolder.rawValue: self = .folder
        case LbLink.rawValue: self = .link(fileType.link_target.toUUID())
        default: fatalError("Unknown file type \(fileType)")
        }
    }
    
    func toLbFileType() -> LbFileType {
        LbFileType(tag: .init(UInt32(self.rawValue)), link_target: lbLinkTarget)
    }
}

extension Array<LbUuid> {
    func toUUIDs() -> [UUID] {
        var ids: [UUID] = []
        
        for id in self {
            ids.append(id.toUUID())
        }
        
        return ids
    }
}

extension UUID {
    func toLbUuid() -> LbUuid {
        LbUuid(bytes: self.uuid)
    }
}

extension LbUuid {
    func toUUID() -> UUID {
        UUID(uuid: self.bytes)
    }
}
