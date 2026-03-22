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
        id = file.id.toUUID()
        parent = file.parent.toUUID()
        name = String(cString: file.name)
        type = FileType(file.typ)
        lastModifiedBy = String(cString: file.lastmod_by)
        lastModified = file.lastmod
        shares = Array(UnsafeBufferPointer(start: file.shares.list, count: Int(file.shares.count))).toShares()
    }

    public var isRoot: Bool {
        parent == id
    }

    public var isFolder: Bool {
        type == .folder
    }

    public static func == (lhs: File, rhs: File) -> Bool {
        lhs.type == rhs.type &&
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

    public static func < (lhs: File, rhs: File) -> Bool {
        if lhs.type == .folder, rhs.type == .document {
            return true
        }

        if rhs.type == .folder, lhs.type == .document {
            return false
        }

        return lhs.name < rhs.name
    }

    public func shareFrom(to: String) -> String? {
        shares.filter { $0.with == to }.first?.by
    }
}

extension [LbFile] {
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
}

public enum ShareMode: Int, Codable, Hashable {
    case write = 0
    case read = 1

    func toLbShareMode() -> Bridge.ShareMode {
        Bridge.ShareMode(UInt32(rawValue))
    }
}

extension [LbShare] {
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
        switch self {
        case .document: 0
        case .folder: 1
        case .link: 2
        }
    }

    var lbLinkTarget: LbUuid {
        switch self {
        case .document: LbUuid(bytes: (0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0))
        case .folder: LbUuid(bytes: (0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0))
        case let .link(id): id.toLbUuid()
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
        LbFileType(tag: .init(UInt32(rawValue)), link_target: lbLinkTarget)
    }
}

extension [LbUuid] {
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
        LbUuid(bytes: uuid)
    }
}

extension LbUuid {
    func toUUID() -> UUID {
        UUID(uuid: bytes)
    }
}
