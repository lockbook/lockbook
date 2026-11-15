import Foundation
import SwiftWorkspace

struct StorageSize: Equatable {
    var synced: UInt64 = 0
    var local: UInt64 = 0

    var delta: Int64 {
        Int64(local) - Int64(synced)
    }

    mutating func add(_ other: StorageSize) {
        synced += other.synced
        local += other.local
    }
}

enum StorageKind: CaseIterable {
    case notes, drawings, images, pdfs, text, other, folders

    static let imageExtensions: Set<String> = [
        "avif", "bmp", "dds", "exr", "ff", "gif", "hdr", "ico", "jpeg", "jpg", "png", "pnm", "qoi",
        "tga", "tiff", "webp", "cr2", "heic", "heif",
    ]

    static let textExtensions: Set<String> = [
        "txt", "c", "h", "cpp", "hpp", "cs", "css", "go", "html", "java", "js", "json", "jsx", "kt",
        "lua", "py", "rb", "rs", "sh", "sql", "swift", "toml", "ts", "tsx", "xml", "yaml", "yml",
        "zig", "csv", "log",
    ]

    init(name: String, type: FileType) {
        guard type == .document else {
            self = .folders
            return
        }
        let ext = name.split(separator: ".").last.map { $0.lowercased() } ?? ""
        switch ext {
        case "md": self = .notes
        case "svg", "draw": self = .drawings
        case "pdf": self = .pdfs
        case _ where Self.imageExtensions.contains(ext): self = .images
        case _ where Self.textExtensions.contains(ext): self = .text
        default: self = .other
        }
    }

    var label: String {
        switch self {
        case .notes: "Notes"
        case .drawings: "Drawings"
        case .images: "Images"
        case .pdfs: "PDFs"
        case .text: "Text & code"
        case .other: "Other files"
        case .folders: "Folders & links"
        }
    }

    var systemImage: String {
        switch self {
        case .notes: "doc.richtext"
        case .drawings: "pencil.and.scribble"
        case .images: "photo"
        case .pdfs: "doc.on.doc"
        case .text: "doc.plaintext"
        case .other: "doc"
        case .folders: "folder"
        }
    }
}

struct StorageKindRow: Identifiable {
    let kind: StorageKind
    var count: Int
    var size: StorageSize

    var id: StorageKind { kind }
}

struct StorageRow: Identifiable {
    let id: UUID
    let name: String
    let isFolder: Bool
    let count: Int
    let size: StorageSize
}

enum StorageChange {
    case new, grew, shrank, movedOut, deleted

    var label: String {
        switch self {
        case .new: "New"
        case .grew, .shrank: "Edited"
        case .movedOut: "Moved to someone else's folder"
        case .deleted: "Deleted"
        }
    }
}

struct StorageChangeRow: Identifiable {
    let fileId: UUID?
    let name: String
    let isFolder: Bool
    let change: StorageChange
    let count: Int
    let size: StorageSize

    var id: String { fileId?.uuidString ?? "deleted" }
}

struct StorageReport {
    static let metadataFee: UInt64 = 1000
    static let largestCount = 12

    let root: UUID
    private var files: [UUID: (name: String, parent: UUID, isFolder: Bool)] = [:]
    private var childIds: [UUID: [UUID]] = [:]
    private var sizes: [UUID: StorageSize] = [:]
    private var totals: [UUID: (size: StorageSize, count: Int)] = [:]
    private(set) var kinds: [StorageKindRow] = []
    private(set) var largest: [StorageRow] = []
    private(set) var changes: [StorageChangeRow] = []

    init(root: UUID, files: [File], entries: [UsageEntry]) {
        self.root = root
        var types: [UUID: StorageKind] = [:]
        for file in files {
            self.files[file.id] = (file.name, file.parent, file.type == .folder)
            types[file.id] = StorageKind(name: file.name, type: file.type)
            if file.id != root {
                childIds[file.parent, default: []].append(file.id)
            }
        }

        var kindRows: [StorageKind: StorageKindRow] = [:]
        var deleted = StorageSize()
        var deletedCount = 0

        for entry in entries {
            let size = StorageSize(synced: entry.synced, local: entry.local)
            sizes[entry.id] = size
            let file = self.files[entry.id]

            if file != nil || entry.local > 0 {
                let kind = types[entry.id] ?? .folders
                kindRows[kind, default: StorageKindRow(kind: kind, count: 0, size: StorageSize())].count += 1
                kindRows[kind]?.size.add(size)
                rollUp(entry.id, size, visible: file != nil)
            } else {
                deletedCount += 1
                deleted.add(size)
                rollUp(entry.id, size, visible: false)
            }

            if let file, entry.synced != entry.local {
                let change: StorageChange =
                    if entry.synced == 0 || (!file.isFolder && entry.synced <= Self.metadataFee) {
                        .new
                    } else if entry.local == 0 {
                        .movedOut
                    } else if entry.local > entry.synced {
                        .grew
                    } else {
                        .shrank
                    }
                changes.append(StorageChangeRow(
                    fileId: entry.id, name: file.name, isFolder: file.isFolder, change: change, count: 1, size: size
                ))
            }
        }

        if deletedCount > 0 {
            let name = deletedCount == 1 ? "1 deleted file" : "\(deletedCount) deleted files"
            changes.append(StorageChangeRow(
                fileId: nil, name: name, isFolder: false, change: .deleted, count: deletedCount, size: deleted
            ))
        }
        changes.sort { abs($0.size.delta) > abs($1.size.delta) }

        kinds = kindRows.values.filter { $0.size.local > 0 }.sorted { $0.size.local > $1.size.local }

        largest = self.files
            .filter { !$0.value.isFolder }
            .compactMap { row($0.key) }
            .filter { $0.size.local > 0 }
            .sorted { $0.size.local > $1.size.local }
            .prefix(Self.largestCount)
            .map { $0 }
    }

    private mutating func rollUp(_ id: UUID, _ size: StorageSize, visible: Bool) {
        var at = visible ? id : root
        var seen = 0
        while true {
            var total = totals[at] ?? (StorageSize(), 0)
            total.size.add(size)
            total.count += 1
            totals[at] = total
            if at == root || seen > files.count {
                break
            }
            seen += 1
            if let parent = files[at]?.parent, files[parent] != nil {
                at = parent
            } else {
                at = root
            }
        }
    }

    func row(_ id: UUID) -> StorageRow? {
        guard let file = files[id] else { return nil }
        if file.isFolder {
            guard let total = totals[id] else { return nil }
            return StorageRow(id: id, name: file.name, isFolder: true, count: total.count, size: total.size)
        }
        guard let size = sizes[id] else { return nil }
        return StorageRow(id: id, name: file.name, isFolder: false, count: 1, size: size)
    }

    func children(of folder: UUID) -> [StorageRow] {
        (childIds[folder] ?? [])
            .compactMap { row($0) }
            .filter { $0.size.local > 0 || $0.size.synced > 0 }
            .sorted { $0.size.local != $1.size.local ? $0.size.local > $1.size.local : $0.name < $1.name }
    }

    func trail(to folder: UUID) -> [(id: UUID, name: String)] {
        var trail: [(id: UUID, name: String)] = []
        var at = folder
        while let file = files[at] {
            trail.append((at, file.name))
            if at == root || trail.count > files.count {
                break
            }
            at = file.parent
        }
        return trail.reversed()
    }

    func parentPath(of id: UUID) -> String {
        guard let parent = files[id]?.parent else { return "" }
        let trail = trail(to: parent)
        if trail.count <= 1 {
            return "/"
        }
        return trail.dropFirst().map(\.name).joined(separator: "/")
    }

    func isFolder(_ id: UUID) -> Bool {
        files[id]?.isFolder ?? false
    }
}
