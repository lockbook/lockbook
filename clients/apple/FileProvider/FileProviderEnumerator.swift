import CryptoKit
import FileProvider

final class FileProviderEnumerator: NSObject, NSFileProviderEnumerator {
    let provider: FileProviderExtension
    let container: NSFileProviderItemIdentifier
    private var baseline: [FileProviderItem]?

    init(provider: FileProviderExtension, container: NSFileProviderItemIdentifier) {
        self.provider = provider
        self.container = container
    }

    func invalidate() {}

    private func items(_ core: FileProviderCore) throws -> [FileProviderItem] {
        if container == .trashContainer { return [] }
        let all = try core.items()
        return all.filter {
            if container == .workingSet { return $0.itemIdentifier != .rootContainer }
            return $0.parentItemIdentifier == container && $0.itemIdentifier != container
        }
    }

    func enumerateItems(for observer: NSFileProviderEnumerationObserver, startingAt page: NSFileProviderPage) {
        provider.queue.async {
            do {
                let items = try self.items(FileProviderCore(domain: self.provider.domain))
                self.baseline = items
                observer.didEnumerate(items)
                observer.finishEnumerating(upTo: nil)
            } catch { observer.finishEnumeratingWithError(error) }
        }
    }

    // Anchors reference durable snapshots, so an extension restart doesn't lose deletions.
    private var snapshots: URL {
        FileManager.default.urls(for: .cachesDirectory, in: .userDomainMask)[0]
            .appendingPathComponent("\(provider.domain.identifier.rawValue)/anchors", isDirectory: true)
    }

    private func save(_ items: [FileProviderItem]) throws -> NSFileProviderSyncAnchor {
        let fm = FileManager.default
        try fm.createDirectory(at: snapshots, withIntermediateDirectories: true)
        let encoder = JSONEncoder()
        encoder.outputFormatting = .sortedKeys
        let data = try encoder.encode(items.sorted { $0.id < $1.id })
        let name = SHA256.hash(data: data).map { String(format: "%02x", $0) }.joined()
        try data.write(to: snapshots.appendingPathComponent(name), options: .atomic)
        // A week of history is enough; expired anchors trigger a complete re-enumeration.
        for url in try fm.contentsOfDirectory(at: snapshots, includingPropertiesForKeys: [.contentModificationDateKey]) {
            if let date = try url.resourceValues(forKeys: [.contentModificationDateKey]).contentModificationDate,
               date < Date().addingTimeInterval(-7 * 86400) { try? fm.removeItem(at: url) }
        }
        return NSFileProviderSyncAnchor(Data(name.utf8))
    }

    func currentSyncAnchor(completionHandler: @escaping (NSFileProviderSyncAnchor?) -> Void) {
        provider.queue.async {
            do {
                let items = try self.baseline ?? self.items(FileProviderCore(domain: self.provider.domain))
                completionHandler(try self.save(items))
            } catch { completionHandler(nil) }
        }
    }

    func enumerateChanges(for observer: NSFileProviderChangeObserver, from anchor: NSFileProviderSyncAnchor) {
        provider.queue.async {
            do {
                guard let name = String(data: anchor.rawValue, encoding: .utf8), name.count == 64, name.allSatisfy({ $0.isHexDigit }),
                      let data = try? Data(contentsOf: self.snapshots.appendingPathComponent(name)),
                      let previous = try? JSONDecoder().decode([FileProviderItem].self, from: data) else {
                    throw NSFileProviderError(.syncAnchorExpired)
                }
                let items = try self.items(FileProviderCore(domain: self.provider.domain))
                let old = Dictionary(uniqueKeysWithValues: previous.map { ($0.id, $0) })
                let ids = Set(items.map(\.id))
                observer.didDeleteItems(withIdentifiers: previous.filter { !ids.contains($0.id) }.map(\.itemIdentifier))
                observer.didUpdate(items.filter { old[$0.id]?.itemVersion != $0.itemVersion })
                observer.finishEnumeratingChanges(upTo: try self.save(items), moreComing: false)
            } catch { observer.finishEnumeratingWithError(error) }
        }
    }
}
