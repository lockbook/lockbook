#if os(macOS)
import AppKit
import FileProvider
import SwiftWorkspace

@Observable final class FinderIntegration {
    static let shared = FinderIntegration()
    var enabled = false
    var busy = false

    private var domain: NSFileProviderDomain? {
        guard let root = try? AppState.lb.listMetadatas().get().first(where: \.isRoot) else { return nil }
        return NSFileProviderDomain(identifier: .init(root.id.uuidString), displayName: "Lockbook")
    }

    func refresh() {
        guard let domain else { return }
        NSFileProviderManager.getDomainsWithCompletionHandler { domains, error in
            DispatchQueue.main.async {
                self.enabled = domains.contains { $0.identifier == domain.identifier }
                if self.enabled {
                    // Refresh the handoff bookmark on each app launch (including after a reboot).
                    do { try self.shareLocation(for: domain) }
                    catch { self.report(error) }
                    NSFileProviderManager(for: domain)?.signalEnumerator(for: .workingSet) { _ in }
                }
            }
        }
    }

    private func shareLocation(for domain: NSFileProviderDomain) throws {
        guard let group = FileManager.default.containerURL(forSecurityApplicationGroupIdentifier: "39ZS78S25U.app.lockbook") else {
            throw CocoaError(.fileWriteNoPermission)
        }
        let location = URL(fileURLWithPath: ProcessInfo.processInfo.environment["LOCKBOOK_PATH"] ?? AppState.LB_LOC)
        let bookmark = try location.bookmarkData(options: [])
        try bookmark.write(to: group.appendingPathComponent("\(domain.identifier.rawValue).bookmark"), options: .atomic)
    }

    func setEnabled(_ value: Bool, completion: (() -> Void)? = nil) {
        guard let domain else { completion?(); return }
        busy = true
        let complete: (Error?) -> Void = { error in
            DispatchQueue.main.async {
                self.busy = false
                if let error { self.report(error) }
                else { self.enabled = value; completion?() }
            }
        }
        if value {
            do { try shareLocation(for: domain) }
            catch { complete(error); return }
            NSFileProviderManager.add(domain, completionHandler: complete)
        } else {
            NSFileProviderManager.getDomainsWithCompletionHandler { domains, error in
                if let error { complete(error) }
                else if domains.contains(where: { $0.identifier == domain.identifier }) {
                    NSFileProviderManager.remove(domain, mode: .preserveDirtyUserData) { _, error in complete(error) }
                } else { complete(nil) }
            }
        }
    }

    func open(id: UUID? = nil) {
        guard let domain else { return }
        let item: NSFileProviderItemIdentifier = id.map { .init($0.uuidString) } ?? .rootContainer
        NSFileProviderManager(for: domain)?.getUserVisibleURL(for: item) { url, error in
            DispatchQueue.main.async {
                if let url {
                    if id != nil { NSWorkspace.shared.activateFileViewerSelecting([url]) }
                    else { NSWorkspace.shared.open(url) }
                } else if let error { self.report(error) }
            }
        }
    }

    private func report(_ error: Error) {
        AppState.shared.error = .custom(title: "Finder", msg: error.localizedDescription)
    }
}
#endif
