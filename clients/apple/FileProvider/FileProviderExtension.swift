import Bridge
import notify
import FileProvider
import OSLog

final class FileProviderExtension: NSObject, NSFileProviderReplicatedExtension {
    let domain: NSFileProviderDomain
    let manager: NSFileProviderManager
    let queue = DispatchQueue(label: "app.lockbook.file-provider")
    private var changeToken = NOTIFY_TOKEN_INVALID
    private var invalidated = false
    private var syncTimer: DispatchSourceTimer?

    required init(domain: NSFileProviderDomain) {
        self.domain = domain
        manager = NSFileProviderManager(for: domain)!
        super.init()
        queue.async { [weak self] in self?.observeChanges() }
        let syncTimer = DispatchSource.makeTimerSource(queue: DispatchQueue(label: "app.lockbook.file-provider.sync"))
        syncTimer.schedule(deadline: .now(), repeating: 30)
        syncTimer.setEventHandler { [weak self] in
            guard let self else { return }
            self.queue.async { self.observeChanges() }
            guard let core = try? FileProviderCore(domain: self.domain) else { return }
            lb_free_err(lb_sync(core.core.lb))
            self.signalChanges()
        }
        syncTimer.resume()
        self.syncTimer = syncTimer
    }

    func invalidate() {
        syncTimer?.cancel()
        syncTimer = nil
        queue.async {
            self.invalidated = true
            if self.changeToken != NOTIFY_TOKEN_INVALID {
                notify_cancel(self.changeToken)
                self.changeToken = NOTIFY_TOKEN_INVALID
            }
        }
    }

    deinit {
        if changeToken != NOTIFY_TOKEN_INVALID { notify_cancel(changeToken) }
    }

    private func observeChanges() {
        guard !invalidated, changeToken == NOTIFY_TOKEN_INVALID else { return }
        do {
            let core = try FileProviderCore(domain: domain)
            let name = lb_change_notification_name(core.core.lb)
            defer { lb_free_str(name) }
            let status = notify_register_dispatch(name, &changeToken, queue) { [weak self] _ in
                guard let self, !self.invalidated else { return }
                self.signalChanges()
            }
            guard status == NOTIFY_STATUS_OK else {
                changeToken = NOTIFY_TOKEN_INVALID
                throw CocoaError(.featureUnsupported)
            }
            // Cover commits before registration, including during extension startup.
            signalChanges()
        } catch {
            Logger(subsystem: "app.lockbook.FileProvider", category: "changes")
                .error("Could not observe Lockbook changes: \(error.localizedDescription, privacy: .public)")
        }
    }

    func signalChanges() {
        manager.signalEnumerator(for: .workingSet) { _ in }
    }

    func perform<T>(_ work: @escaping (FileProviderCore) throws -> T,
                    completion: @escaping (Result<T, Error>) -> Void) -> Progress {
        let progress = Progress(totalUnitCount: 1)
        queue.async {
            let result = Result {
                guard !progress.isCancelled else { throw CocoaError(.userCancelled) }
                return try work(FileProviderCore(domain: self.domain))
            }
            progress.completedUnitCount = 1
            completion(result)
        }
        return progress
    }

    func item(for identifier: NSFileProviderItemIdentifier, request: NSFileProviderRequest,
              completionHandler: @escaping (NSFileProviderItem?, Error?) -> Void) -> Progress {
        perform({ try $0.item(identifier) }) {
            switch $0 {
            case .success(let item): completionHandler(item, nil)
            case .failure(let error): completionHandler(nil, error)
            }
        }
    }

    func fetchContents(for identifier: NSFileProviderItemIdentifier, version: NSFileProviderItemVersion?,
                       request: NSFileProviderRequest,
                       completionHandler: @escaping (URL?, NSFileProviderItem?, Error?) -> Void) -> Progress {
        perform({ core in
            let item = try core.item(identifier)
            let data = try core.read(item)
            if let version, version.contentVersion != item.contentVersion {
                throw NSFileProviderError(.versionNoLongerAvailable)
            }
            let url = try self.manager.temporaryDirectoryURL().appendingPathComponent(UUID().uuidString)
            try data.write(to: url)
            return (url, item)
        }) {
            switch $0 {
            case .success(let (url, item)): self.signalChanges(); completionHandler(url, item, nil)
            case .failure(let error): completionHandler(nil, nil, error)
            }
        }
    }

    func createItem(basedOn template: NSFileProviderItem, fields: NSFileProviderItemFields, contents: URL?,
                    options: NSFileProviderCreateItemOptions, request: NSFileProviderRequest,
                    completionHandler: @escaping (NSFileProviderItem?, NSFileProviderItemFields, Bool, Error?) -> Void) -> Progress {
        perform({ core in
            let folder = template.contentType == .folder
            guard folder || template.contentType?.conforms(to: .data) == true else {
                throw CocoaError(.featureUnsupported)
            }
            let data = try contents.map { try Data(contentsOf: $0) }
            // A crash/reimport can replay creation. Reuse only an identical item.
            if options.contains(.mayAlreadyExist),
               let existing = try core.items(in: template.parentItemIdentifier).first(where: {
                   $0.filename == template.filename && $0.folder == folder
               }), try folder || core.read(existing) == (data ?? Data()) {
                return existing
            }
            return try core.create(name: template.filename, parent: template.parentItemIdentifier, folder: folder, data: data)
        }) {
            switch $0 {
            case .success(let item): self.signalChanges(); completionHandler(item, [], false, nil)
            case .failure(let error): completionHandler(nil, [], false, error)
            }
        }
    }

    func modifyItem(_ item: NSFileProviderItem, baseVersion: NSFileProviderItemVersion,
                    changedFields: NSFileProviderItemFields, contents: URL?, options: NSFileProviderModifyItemOptions,
                    request: NSFileProviderRequest,
                    completionHandler: @escaping (NSFileProviderItem?, NSFileProviderItemFields, Bool, Error?) -> Void) -> Progress {
        perform({ core in
            let uuid = LbUuid(bytes: try core.uuid(item.itemIdentifier).uuid)
            if !changedFields.intersection([.filename, .parentItemIdentifier]).isEmpty,
               let collision = try core.items(in: item.parentItemIdentifier).first(where: {
                   $0.itemIdentifier != item.itemIdentifier && $0.filename == item.filename
               }) {
                throw NSError.fileProviderErrorForCollision(with: collision)
            }
            if changedFields.contains(.contents) {
                guard let contents else { throw CocoaError(.fileReadUnknown) }
                try core.write(item.itemIdentifier, version: baseVersion.contentVersion, data: Data(contentsOf: contents))
            }
            if changedFields.contains(.filename) {
                let error = lb_rename_file(core.core.lb, uuid, item.filename)
                defer { lb_free_err(error) }
                try core.check(error)
            }
            if changedFields.contains(.parentItemIdentifier) {
                let parent = LbUuid(bytes: try core.uuid(item.parentItemIdentifier).uuid)
                let error = lb_move_file(core.core.lb, uuid, parent)
                defer { lb_free_err(error) }
                try core.check(error)
            }
            return try core.item(item.itemIdentifier)
        }) {
            switch $0 {
            case .success(let updated):
                self.signalChanges()
                let supported: NSFileProviderItemFields = [.contents, .filename, .parentItemIdentifier, .contentModificationDate]
                completionHandler(updated, changedFields.subtracting(supported), false, nil)
            case .failure(let error): completionHandler(nil, [], false, error)
            }
        }
    }

    func deleteItem(identifier: NSFileProviderItemIdentifier, baseVersion: NSFileProviderItemVersion,
                    options: NSFileProviderDeleteItemOptions, request: NSFileProviderRequest,
                    completionHandler: @escaping (Error?) -> Void) -> Progress {
        perform({ core in
            if !options.contains(.recursive), try core.items().contains(where: { $0.parentItemIdentifier == identifier && $0.itemIdentifier != identifier }) {
                throw NSFileProviderError(.directoryNotEmpty)
            }
            let error = lb_delete_file(core.core.lb, LbUuid(bytes: try core.uuid(identifier).uuid))
            defer { lb_free_err(error) }
            if error?.pointee.code != FileNonexistent { try core.check(error) }
        }) {
            switch $0 {
            case .success: self.signalChanges(); completionHandler(nil)
            case .failure(let error): completionHandler(error)
            }
        }
    }

    func enumerator(for containerItemIdentifier: NSFileProviderItemIdentifier, request: NSFileProviderRequest) throws -> NSFileProviderEnumerator {
        FileProviderEnumerator(provider: self, container: containerItemIdentifier)
    }
}
