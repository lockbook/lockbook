import CryptoKit
import FileProvider

final class FileProviderEnumerator: NSObject, NSFileProviderEnumerator {
    let provider: FileProviderExtension
    let container: NSFileProviderItemIdentifier
    private var baseline: [FileProviderItem]?
    private let queue = DispatchQueue(label: "app.lockbook.file-provider.enumerator")

    init(provider: FileProviderExtension, container: NSFileProviderItemIdentifier) {
        self.provider = provider
        self.container = container
    }

    func invalidate() {}

    private func items(_ core: FileProviderCore) throws -> [FileProviderItem] {
        if container == .trashContainer { return [] }
        if container != .rootContainer && container != .workingSet {
            let item = try core.item(container)
            if !item.folder { return [item] }
        }
        return try core.items(in: container == .workingSet ? nil : container)
            .filter { $0.itemIdentifier != .rootContainer }
    }

    func enumerateItems(for observer: NSFileProviderEnumerationObserver, startingAt page: NSFileProviderPage) {
        queue.async {
            do {
                let items = try self.items(self.provider.core())
                self.baseline = items
                if self.container != .workingSet { self.provider.signalChanges() }
                observer.didEnumerate(items)
                observer.finishEnumerating(upTo: nil)
            } catch { observer.finishEnumeratingWithError(error) }
        }
    }

    private func save(_ items: [FileProviderItem]) throws -> NSFileProviderSyncAnchor {
        let encoder = JSONEncoder()
        encoder.outputFormatting = .sortedKeys
        let data = try encoder.encode(items.sorted { $0.id < $1.id })
        return provider.snapshots.save(data)
    }

    func currentSyncAnchor(completionHandler: @escaping (NSFileProviderSyncAnchor?) -> Void) {
        queue.async {
            do {
                let items = try self.baseline ?? self.items(self.provider.core())
                completionHandler(try self.save(items))
            } catch { completionHandler(nil) }
        }
    }

    func enumerateChanges(for observer: NSFileProviderChangeObserver, from anchor: NSFileProviderSyncAnchor) {
        queue.async {
            do {
                guard let data = self.provider.snapshots.load(anchor),
                      let previous = try? JSONDecoder().decode([FileProviderItem].self, from: data) else {
                    throw NSFileProviderError(.syncAnchorExpired)
                }
                let items = try self.items(self.provider.core())
                let old = Dictionary(uniqueKeysWithValues: previous.map { ($0.id, $0) })
                let ids = Set(items.map(\.id))
                observer.didDeleteItems(withIdentifiers: previous.filter { !ids.contains($0.id) }.map(\.itemIdentifier))
                observer.didUpdate(items.filter { old[$0.id]?.itemVersion != $0.itemVersion })
                observer.finishEnumeratingChanges(upTo: try self.save(items), moreComing: false)
            } catch { observer.finishEnumeratingWithError(error) }
        }
    }
}

// Shared by enumerators; restarting the extension expires its anchors.
final class FileProviderSnapshots {
    private let lock = NSLock()
    private var entries: [Data: (data: Data, saved: Date)] = [:]

    func save(_ data: Data) -> NSFileProviderSyncAnchor {
        let anchor = Data(SHA256.hash(data: data))
        lock.withLock {
            let now = Date()
            entries = entries.filter { $0.value.saved >= now.addingTimeInterval(-7 * 86400) }
            entries[anchor] = (data, now)
        }
        return NSFileProviderSyncAnchor(anchor)
    }

    func load(_ anchor: NSFileProviderSyncAnchor) -> Data? {
        lock.withLock { entries[anchor.rawValue]?.data }
    }
}
