import Bridge
import FileProvider

// Each operation opens a fresh database view. File locks and safe_write arbitrate
// with the app/CLI; this short-lived core never starts another background syncer.
final class FileProviderCore {
    let core: LbInitRes
    let root: UUID
    private let location: URL
    private struct Size: Codable { let version: Data; let count: Int }
    private var sizes: [String: Size] = [:]
    private let sizesURL: URL
    private var sizesChanged = false
    private var folders: Set<String> = [NSFileProviderItemIdentifier.rootContainer.rawValue]
    private let foldersURL: URL
    private var foldersChanged = false

    init(domain: NSFileProviderDomain) throws {
        let fm = FileManager.default
        let cache = fm.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
        try fm.createDirectory(at: cache, withIntermediateDirectories: true)
        sizesURL = cache.appendingPathComponent("\(domain.identifier.rawValue)-sizes.json")
        foldersURL = cache.appendingPathComponent("\(domain.identifier.rawValue)-folders.json")
        if let data = try? Data(contentsOf: sizesURL) {
            sizes = (try? JSONDecoder().decode([String: Size].self, from: data)) ?? [:]
        }
        if let data = try? Data(contentsOf: foldersURL), let saved = try? JSONDecoder().decode(Set<String>.self, from: data) {
            folders.formUnion(saved)
        }
        let saved = cache.appendingPathComponent("\(domain.identifier.rawValue).bookmark")
        var stale = false
        let handoff = fm.containerURL(forSecurityApplicationGroupIdentifier: "39ZS78S25U.app.lockbook")
            .flatMap { try? Data(contentsOf: $0.appendingPathComponent("\(domain.identifier.rawValue).bookmark")) }
        let sharedLocation = handoff.flatMap { try? URL(resolvingBookmarkData: $0, bookmarkDataIsStale: &stale) }
        if let bookmark = try? Data(contentsOf: saved),
           let url = try? URL(resolvingBookmarkData: bookmark, options: [.withSecurityScope], bookmarkDataIsStale: &stale), !stale,
           sharedLocation == nil || sharedLocation?.path == url.path {
            location = url
            _ = location.startAccessingSecurityScopedResource()
        } else {
            guard let sharedLocation else { throw NSFileProviderError(.notAuthenticated) }
            location = sharedLocation
            // Convert the app's implicit access grant into this extension's persistent grant.
            let persistent = try location.bookmarkData(options: [.withSecurityScope])
            try persistent.write(to: saved, options: .atomic)
        }
        guard fm.fileExists(atPath: location.appendingPathComponent("CoreV5").path),
              let root = UUID(uuidString: domain.identifier.rawValue) else {
            location.stopAccessingSecurityScopedResource()
            throw NSFileProviderError(.notAuthenticated)
        }
        self.root = root
        let initialized = lb_init(location.path, false, false)
        if initialized.err != nil {
            let error = Self.error(initialized.err!)
            lb_free_init(initialized)
            location.stopAccessingSecurityScopedResource()
            throw error
        }
        let result = lb_get_root(initialized.lb)
        defer { lb_free_file_res(result) }
        guard result.err == nil, UUID(uuid: result.file.id.bytes) == root else {
            lb_free_init(initialized)
            location.stopAccessingSecurityScopedResource()
            throw NSFileProviderError(.notAuthenticated)
        }
        core = initialized
    }

    deinit {
        if sizesChanged, let data = try? JSONEncoder().encode(sizes) {
            try? data.write(to: sizesURL, options: .atomic)
        }
        if foldersChanged, let data = try? JSONEncoder().encode(folders) {
            try? data.write(to: foldersURL, options: .atomic)
        }
        lb_free_init(core)
        location.stopAccessingSecurityScopedResource()
    }

    func uuid(_ id: NSFileProviderItemIdentifier) throws -> UUID {
        if id == .rootContainer { return root }
        guard let uuid = UUID(uuidString: id.rawValue) else { throw NSFileProviderError(.noSuchItem) }
        return uuid
    }

    func identifier(_ uuid: UUID) -> String {
        uuid == root ? NSFileProviderItemIdentifier.rootContainer.rawValue : uuid.uuidString
    }

    func item(_ id: NSFileProviderItemIdentifier) throws -> FileProviderItem {
        let result = lb_get_file(core.lb, LbUuid(bytes: try uuid(id).uuid))
        defer { lb_free_file_res(result) }
        try check(result.err)
        return try item(result.file)
    }

    private func item(_ file: LbFile, loadSize: Bool = true) throws -> FileProviderItem {
        let id = UUID(uuid: file.id.bytes)
        let folder = file.typ.tag == LbFolder
        let version = lb_document_version(core.lb, file.id)
        defer { lb_free_err(version.err) }
        try check(version.err)
        let item = FileProviderItem(id: identifier(id), parent: identifier(UUID(uuid: file.parent.bytes)),
                                name: id == root ? "Lockbook" : String(cString: file.name), folder: folder,
                                modified: file.lastmod, contentVersion: Self.data(version.version))
        if !folder {
            if !version.version.present {
                item.size = 0
            } else if let cached = sizes[item.id], cached.version == item.contentVersion {
                item.size = cached.count
            } else if loadSize || sizes[item.id] != nil {
                // Finder treats a nil size as an empty file. Resolve sizes only for a
                // folder being browsed, or a previously enumerated document that changed.
                _ = try read(item)
            }
        }
        return item
    }

    func items(in parent: NSFileProviderItemIdentifier? = nil) throws -> [FileProviderItem] {
        let result: LbFileListRes
        if let parent {
            result = lb_get_children(core.lb, LbUuid(bytes: try uuid(parent).uuid))
            foldersChanged = folders.insert(parent.rawValue).inserted || foldersChanged
        } else {
            result = lb_get_and_get_children_recursively(core.lb, LbUuid(bytes: root.uuid))
        }
        defer { lb_free_file_list_res(result) }
        try check(result.err)
        return try UnsafeBufferPointer(start: result.list.list, count: Int(result.list.count))
            .map { try item($0, loadSize: folders.contains(identifier(UUID(uuid: $0.parent.bytes)))) }
    }

    func read(_ item: FileProviderItem) throws -> Data {
        var version = LbDocumentVersion()
        let result = lb_read_document_versioned(core.lb, LbUuid(bytes: try uuid(item.itemIdentifier).uuid), &version)
        defer { lb_free_doc_res(result) }
        try check(result.err)
        item.contentVersion = Self.data(version)
        item.size = Int(result.len)
        sizes[item.id] = Size(version: item.contentVersion, count: Int(result.len))
        sizesChanged = true
        return Data(bytes: result.doc, count: Int(result.len))
    }

    func write(_ id: NSFileProviderItemIdentifier, version: Data, data: Data) throws {
        guard version.count == 33 else { throw NSFileProviderError(.cannotSynchronize) }
        var expected = LbDocumentVersion()
        expected.present = version[0] != 0
        withUnsafeMutableBytes(of: &expected.hmac) { $0.copyBytes(from: version.dropFirst()) }
        let uuid = try uuid(id)
        let result = data.withUnsafeBytes {
            lb_write_document_versioned(core.lb, LbUuid(bytes: uuid.uuid), expected,
                                        $0.bindMemory(to: UInt8.self).baseAddress, UInt(data.count))
        }
        defer { lb_free_err(result.err) }
        try check(result.err)
        sizes[identifier(uuid)] = Size(version: Self.data(result.version), count: data.count)
        sizesChanged = true
    }

    func create(name: String, parent: NSFileProviderItemIdentifier, folder: Bool, data: Data?) throws -> FileProviderItem {
        let parentUUID = try uuid(parent)
        let result = lb_create_file(core.lb, name, LbUuid(bytes: parentUUID.uuid),
                                    LbFileType(tag: folder ? LbFolder : LbDocument, link_target: LbUuid()))
        defer { lb_free_file_res(result) }
        if let error = result.err, error.pointee.code == PathTaken,
           let existing = try items(in: parent).first(where: { $0.filename == name }) {
            throw NSError.fileProviderErrorForCollision(with: existing)
        }
        try check(result.err)
        let created = try item(result.file)
        if !folder, let data {
            do { try write(created.itemIdentifier, version: created.contentVersion, data: data) }
            catch {
                lb_free_err(lb_delete_file(core.lb, result.file.id))
                throw error
            }
        }
        return try item(created.itemIdentifier)
    }

    func check(_ error: UnsafeMutablePointer<LbFfiErr>?) throws {
        if let error { throw Self.error(error) }
    }

    static func error(_ error: UnsafeMutablePointer<LbFfiErr>) -> Error {
        switch error.pointee.code {
        case AccountNonexistent, RootNonexistent: return NSFileProviderError(.notAuthenticated)
        case FileNonexistent, FileParentNonexistent: return NSFileProviderError(.noSuchItem)
        case ServerUnreachable: return NSFileProviderError(.serverUnreachable)
        case ReReadRequired:
            return NSError(domain: NSFileProviderErrorDomain, code: NSFileProviderError.cannotSynchronize.rawValue,
                           userInfo: [NSLocalizedDescriptionKey: "This file changed in Lockbook. Copy your Finder edits to a new file to keep both versions."])
        case InsufficientPermission: return CocoaError(.fileWriteNoPermission)
        case UsageIsOverDataCap, UsageIsOverFreeTierDataCap: return NSFileProviderError(.insufficientQuota)
        default: return NSError(domain: NSCocoaErrorDomain, code: NSFileWriteUnknownError,
                                userInfo: [NSLocalizedDescriptionKey: String(cString: error.pointee.msg)])
        }
    }

    static func data(_ version: LbDocumentVersion) -> Data {
        var hmac = version.hmac
        return Data([version.present ? 1 : 0]) + withUnsafeBytes(of: &hmac) { Data($0) }
    }
}
