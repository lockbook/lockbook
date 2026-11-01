import Bridge
import FileProvider

// Keep one core for the extension's lifetime and refresh its database between callbacks.
// File locks and safe_write coordinate with other callbacks and the app/CLI.
final class FileProviderCore {
    let core: LbInitRes
    let root: UUID
    let cache: FileProviderCache
    let ipcDirectory: URL

    init(domain: NSFileProviderDomain, cache: FileProviderCache) throws {
        self.cache = cache
        let fm = FileManager.default
        guard let group = fm.containerURL(forSecurityApplicationGroupIdentifier: "group.app.lockbook") else {
            throw NSFileProviderError(.notAuthenticated)
        }
        let location = group.appendingPathComponent("data", isDirectory: true)
        ipcDirectory = location.appendingPathComponent("CoreV5/ipc", isDirectory: true)
        guard fm.fileExists(atPath: location.appendingPathComponent("CoreV5").path),
              let root = UUID(uuidString: domain.identifier.rawValue) else {
            throw NSFileProviderError(.notAuthenticated)
        }
        self.root = root
        let initialized = lb_init(location.path, "file-provider", BuildInfo.identity, true, false)
        if initialized.err != nil {
            let error = Self.error(initialized.err!)
            lb_free_init(initialized)
            throw error
        }
        let result = lb_get_root(initialized.lb)
        defer { lb_free_file_res(result) }
        guard result.err == nil, UUID(uuid: result.file.id.bytes) == root else {
            lb_free_init(initialized)
            throw NSFileProviderError(.notAuthenticated)
        }
        core = initialized
    }

    deinit {
        lb_free_init(core)
    }

    func uuid(_ id: NSFileProviderItemIdentifier) throws -> UUID {
        if id == .rootContainer { return root }
        guard let uuid = UUID(uuidString: id.rawValue) else { throw NSFileProviderError(.noSuchItem) }
        return uuid
    }

    func identifier(_ uuid: UUID) -> String {
        uuid == root ? NSFileProviderItemIdentifier.rootContainer.rawValue : uuid.uuidString
    }

    func item(_ id: NSFileProviderItemIdentifier, loadSize: Bool = true) throws -> FileProviderItem {
        let result = lb_get_file(core.lb, LbUuid(bytes: try uuid(id).uuid))
        defer { lb_free_file_res(result) }
        try check(result.err)
        return try item(result.file, loadSize: loadSize)
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
        if !folder && !version.version.present { item.size = 0 }
        cache.update(item)
        if !folder && item.size == nil && loadSize {
            _ = try read(item)
        }
        return item
    }

    func items(in parent: NSFileProviderItemIdentifier? = nil, loadSizes: Bool = true) throws -> [FileProviderItem] {
        let result: LbFileListRes
        if let parent {
            result = lb_get_children(core.lb, LbUuid(bytes: try uuid(parent).uuid))
        } else {
            result = lb_get_and_get_children_recursively(core.lb, LbUuid(bytes: root.uuid))
        }
        defer { lb_free_file_list_res(result) }
        try check(result.err)
        let files = UnsafeBufferPointer(start: result.list.list, count: Int(result.list.count))
        let workers = min(files.count, 4)
        let lock = NSLock()
        var items = [Result<FileProviderItem, Error>?](repeating: nil, count: files.count)
        DispatchQueue.concurrentPerform(iterations: workers) { worker in
            for index in stride(from: worker, to: files.count, by: workers) {
                let item = Result { try self.item(files[index], loadSize: loadSizes) }
                lock.withLock { items[index] = item }
            }
        }
        return try items.map { try $0!.get() }
    }

    func read(_ item: FileProviderItem) throws -> Data {
        var version = LbDocumentVersion()
        let result = lb_read_document_versioned(core.lb, LbUuid(bytes: try uuid(item.itemIdentifier).uuid), &version)
        defer { lb_free_doc_res(result) }
        try check(result.err)
        if item.contentVersion != Self.data(version) {
            // The document changed between the metadata lookup and its read.
            let current = try self.item(item.itemIdentifier, loadSize: false)
            guard current.contentVersion == Self.data(version) else {
                throw NSFileProviderError(.versionNoLongerAvailable)
            }
            item.contentVersion = current.contentVersion
            item.modified = current.modified
            item.modifiedDate = current.modifiedDate
        }
        item.size = Int(result.len)
        let data = Data(bytes: result.doc, count: Int(result.len))
        cache.update(item)
        return data
    }

    @discardableResult
    func write(_ id: NSFileProviderItemIdentifier, version: Data, data: Data) throws -> FileProviderItem {
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
        let updated = try item(id, loadSize: false)
        if updated.contentVersion == Self.data(result.version) {
            updated.size = data.count
            cache.update(updated)
        }
        return updated
    }

    func create(name: String, parent: NSFileProviderItemIdentifier, folder: Bool, data: Data?) throws -> FileProviderItem {
        let parentUUID = try uuid(parent)
        let result = lb_create_file(core.lb, name, LbUuid(bytes: parentUUID.uuid),
                                    LbFileType(tag: folder ? LbFolder : LbDocument, link_target: LbUuid()))
        defer { lb_free_file_res(result) }
        if let error = result.err, error.pointee.code == PathTaken,
           let existing = try items(in: parent, loadSizes: false).first(where: { $0.filename == name }) {
            throw NSError.fileProviderErrorForCollision(with: existing)
        }
        try check(result.err)
        let created = try item(result.file)
        if !folder, let data {
            do { return try write(created.itemIdentifier, version: created.contentVersion, data: data) }
            catch {
                lb_free_err(lb_delete_file(core.lb, result.file.id))
                throw error
            }
        }
        return created
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

// Only bookkeeping is locked; decryption, network requests and writes run outside it.
// This cache intentionally resets with the extension, including its timestamp overrides.
final class FileProviderCache {
    private struct Entry {
        let modified: UInt64
        let version: Data
        let date: Date
        let size: Int?
    }
    private let lock = NSLock()
    private var entries: [String: Entry] = [:]

    func update(_ item: FileProviderItem, date: Date? = nil) {
        lock.withLock {
            let previous = entries[item.id]
            if previous?.version == item.contentVersion {
                item.modifiedDate = previous?.date
                item.size = item.size ?? previous?.size
            } else {
                item.modifiedDate = nil
            }
            if let date { item.modifiedDate = date }
            // A slow request must not roll a newer observation back.
            if let previous, previous.modified > item.modified, previous.version != item.contentVersion { return }
            entries[item.id] = Entry(modified: max(item.modified, previous?.modified ?? 0), version: item.contentVersion,
                                     date: item.contentModificationDate!, size: item.size)
        }
    }
}
