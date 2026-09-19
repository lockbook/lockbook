#if os(macOS)
import AppKit
import Bridge
import Observation
import Security

/// Owns one listener per app, regardless of the number of workspace windows.
/// All methods and the command pump run on the main thread.
@Observable
public final class ExperimentalMcp {
    public static let shared = ExperimentalMcp()
    public private(set) var status = "Stopped"
    public private(set) var running = false
    public private(set) var error: String?

    @ObservationIgnored private var views = NSHashTable<MacMTK>.weakObjects()
    @ObservationIgnored private var timer: Timer?
    @ObservationIgnored private var observer: NSObjectProtocol?
    @ObservationIgnored private var appliedPort: UInt16?
    @ObservationIgnored private var appliedToken: String?
    @ObservationIgnored private weak var lastView: MacMTK?
    private static let service = "net.lockbook.experimental-mcp"
    private static let account = "local-access-token"

    private init() {
        observer = NotificationCenter.default.addObserver(
            forName: UserDefaults.didChangeNotification, object: nil, queue: .main
        ) { [weak self] _ in self?.refresh() }
    }

    func register(_ view: MacMTK) {
        views.add(view)
        lastView = view
        refresh()
    }

    func unregister(_ view: MacMTK) {
        views.remove(view)
        if lastView === view { lastView = nil }
        refresh()
    }

    public func refresh() {
        precondition(Thread.isMainThread)
        let enabled = UserDefaults.standard.bool(forKey: "experimentalMcpEnabled")
        guard enabled, !views.allObjects.isEmpty else {
            stop()
            status = enabled ? "Open a Lockbook workspace to start" : "Stopped"
            return
        }
        let configured = UserDefaults.standard.object(forKey: "experimentalMcpPort") as? Int ?? 19687
        guard let port = UInt16(exactly: configured), port >= 1024 else {
            stop()
            error = "Choose a port from 1024 to 65535."
            status = "Unable to start"
            return
        }
        do {
            let token = try loadOrCreateToken()
            if running, appliedPort == port, appliedToken == token { return }
            if let message = configure_mcp(true, port, token) {
                let text = String(cString: message)
                free_text(message)
                stop()
                error = text
                status = "Unable to start"
                return
            }
            appliedPort = port
            appliedToken = token
            running = true
            error = nil
            status = "Listening on http://127.0.0.1:\(port)/mcp"
            if timer == nil {
                let timer = Timer(timeInterval: 0.1, repeats: true) { [weak self] _ in self?.pump() }
                RunLoop.main.add(timer, forMode: .common)
                self.timer = timer
            }
        } catch {
            stop()
            self.error = error.localizedDescription
            status = "Unable to start"
        }
    }

    private func stop() {
        if let message = configure_mcp(false, 0, "") { free_text(message) }
        timer?.invalidate()
        timer = nil
        running = false
        appliedPort = nil
        appliedToken = nil
        error = nil
    }

    private func pump() {
        let available = views.allObjects
        guard let view = available.first(where: { $0.window?.isKeyWindow == true })
            ?? lastView ?? available.first else {
            refresh()
            return
        }
        lastView = view
        guard let handle = view.wsHandle else { return }
        process_mcp(handle)
    }

    /// Copies explicit credentials so desktop clients need no inherited shell environment.
    public func copyConfiguration() {
        guard running, let port = appliedPort, let token = appliedToken else { return }
        let config = """
        [mcp_servers.lockbook]
        url = "http://127.0.0.1:\(port)/mcp"
        http_headers = { Authorization = "Bearer \(token)" }
        """
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(config, forType: .string)
    }

    public func resetAccessToken() {
        stop() // Revoke existing connections before changing the credential.
        let result = SecItemDelete(keychainQuery() as CFDictionary)
        guard result == errSecSuccess || result == errSecItemNotFound else {
            error = "Cannot reset MCP token: \(result)"
            status = "Unable to reset access token"
            return
        }
        refresh()
    }

    private func keychainQuery() -> [String: Any] {
        [kSecClass as String: kSecClassGenericPassword,
         kSecAttrService as String: Self.service,
         kSecAttrAccount as String: Self.account]
    }

    private func loadOrCreateToken() throws -> String {
        // Avoid a Keychain read on every unrelated UserDefaults notification.
        if let token = appliedToken { return token }
        var query = keychainQuery()
        query[kSecReturnData as String] = true
        query[kSecMatchLimit as String] = kSecMatchLimitOne
        var item: CFTypeRef?
        let result = SecItemCopyMatching(query as CFDictionary, &item)
        if result == errSecSuccess, let data = item as? Data,
           let token = String(data: data, encoding: .utf8), token.count == 64,
           token.allSatisfy({ $0.isHexDigit }) { return token }
        guard result == errSecItemNotFound else {
            throw NSError(domain: "LockbookMCP", code: Int(result), userInfo: [NSLocalizedDescriptionKey: "Cannot read MCP token from Keychain (\(result)). Try Reset access token."])
        }
        var bytes = [UInt8](repeating: 0, count: 32)
        let randomResult = SecRandomCopyBytes(kSecRandomDefault, bytes.count, &bytes)
        guard randomResult == errSecSuccess else { throw NSError(domain: NSOSStatusErrorDomain, code: Int(randomResult)) }
        let token = bytes.map { String(format: "%02x", $0) }.joined()
        var entry = keychainQuery()
        entry[kSecValueData as String] = Data(token.utf8)
        entry[kSecAttrAccessible as String] = kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly
        let addResult = SecItemAdd(entry as CFDictionary, nil)
        guard addResult == errSecSuccess else { throw NSError(domain: NSOSStatusErrorDomain, code: Int(addResult)) }
        return token
    }
}
#endif
