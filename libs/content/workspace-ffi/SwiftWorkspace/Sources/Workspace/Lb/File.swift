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
    
    public var isRoot: Bool { parent == id }
    
    init(_ file: LbFile) {
        self.id = file.id.toUUID()
        self.parent = file.parent.toUUID()
        self.name = String(cString: file.name)
        self.type = FileType(rawValue: Int(file.typ.tag.rawValue))!
        self.lastModifiedBy = String(cString: file.lastmod_by)
        self.lastModified = file.lastmod
        self.shares = Array(UnsafeBufferPointer(start: file.shares.list, count: Int(file.shares.count))).toShares()
    }
    
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

public enum ShareMode: Int, Codable {
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

public enum FileType: Int, Codable {
    case document = 0
    case folder = 1
    
    func toLbFileType() -> LbFileType {
        LbFileType(tag: .init(UInt32(self.rawValue)), link_target: LbUuid(bytes: (0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0 )))
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
