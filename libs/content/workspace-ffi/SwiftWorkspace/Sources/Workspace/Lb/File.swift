import Bridge
import Foundation

public struct File {
    let id: UUID
    let parent: UUID
    let name: String
    let type: FileType
    let lastModifiedBy: String
    let lastModified: UInt64
    let shares: [Share]
    
    init(_ file: LbFile) {
        self.id = file.id.toUUID()
        self.parent = file.parent.toUUID()
        self.name = String(cString: file.name)
        self.type = FileType(rawValue: Int(file.typ.tag.rawValue))!
        self.lastModifiedBy = String(cString: file.lastmod_by)
        self.lastModified = file.lastmod
        self.shares = Array(UnsafeBufferPointer(start: file.shares.list, count: Int(file.shares.count))).toShares()
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

public struct Share {
    let by: String
    let with: String
    let mode: ShareMode
    
    init(by: String, with: String, mode: ShareMode) {
        self.by = by
        self.with = with
        self.mode = mode
    }
}

extension Array<LbShare> {
    func toShares() -> [Share] {
        var shares: [Share] = []
        
        for share in self {
            shares.append(Share(by: String(cString: share.by), with: String(cString: share.with), mode: share.mode))
        }
        
        return shares
    }
}

public enum FileType: Int {
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
