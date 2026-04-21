import SwiftUI
import SwiftWorkspace

class AppState: ObservableObject {
    static let shared = AppState()

    static let LB_LOC: String = {
        #if os(macOS)
            NSHomeDirectory() + "/.lockbook"
        #else
            resolveIOSWritablePath()
        #endif
    }()

    #if !os(macOS)
    private static func resolveIOSWritablePath() -> String {
        let fm = FileManager.default
        let legacyURL = fm.urls(for: .documentDirectory, in: .userDomainMask).last!

        guard let groupURL = fm.containerURL(forSecurityApplicationGroupIdentifier: "group.app.lockbook") else {
            return legacyURL.path
        }
        let newURL = groupURL.appendingPathComponent("lockbook", isDirectory: true)

        migrateLegacyDataIfNeeded(from: legacyURL, to: newURL)

        return newURL.path
    }

    private static func migrateLegacyDataIfNeeded(from legacyURL: URL, to newURL: URL) {
        let fm = FileManager.default

        if (try? fm.createDirectory(at: newURL, withIntermediateDirectories: true)) == nil {
            return
        }

        let legacyContents: [String]
        do {
            legacyContents = try fm.contentsOfDirectory(atPath: legacyURL.path)
        } catch {
            return
        }

        for name in legacyContents {
            let src = legacyURL.appendingPathComponent(name)
            let dst = newURL.appendingPathComponent(name)
            if fm.fileExists(atPath: dst.path) { continue }
            do {
                try fm.moveItem(at: src, to: dst)
            } catch {
                do {
                    try fm.copyItem(at: src, to: dst)
                    try fm.removeItem(at: src)
                } catch {
                    continue
                }
            }
        }
    }
    #endif

    static let LB_API_URL: String? = ProcessInfo.processInfo.environment["API_LOCATION"]

    static let lb: LbAPI = {
        if isPreviewEnvironmentKey.defaultValue {
            return MockLb()
        }

        return Lb(writablePath: ProcessInfo.processInfo.environment["LOCKBOOK_PATH"] ?? LB_LOC, logs: true)
    }()

    static var billingState: BillingState = .init()

    @Published var account: Account? = nil
    @Published var isLoggedIn: Bool = false
    @Published var error: UIError? = nil

    private init() {
        checkIfLoggedIn()
    }

    func checkIfLoggedIn() {
        switch AppState.lb.getAccount() {
        case let .success(account):
            isLoggedIn = true
            self.account = account
        case .failure:
            isLoggedIn = false
            account = nil
        }
    }
}

enum UIError: Identifiable {
    case lb(error: LbError)
    case custom(title: String, msg: String)

    var id: String {
        switch self {
        case let .lb(error): "lb-\(error.msg)"
        case let .custom(title, _): "custom-\(title)"
        }
    }

    var title: String {
        switch self {
        case .lb: "Error"
        case let .custom(title, _): title
        }
    }

    var message: String {
        switch self {
        case let .lb(error): error.msg
        case let .custom(_, msg): msg
        }
    }
}
