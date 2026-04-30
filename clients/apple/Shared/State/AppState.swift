import Foundation
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
    // migration from 26.4.11 to app directory
    private static func resolveIOSWritablePath() -> String {
        let fm = FileManager.default
        let legacyURL = fm.urls(for: .documentDirectory, in: .userDomainMask).last!

        guard let groupURL = fm.containerURL(forSecurityApplicationGroupIdentifier: "group.app.lockbook") else {
            return legacyURL.path
        }
        let newURL = groupURL.appendingPathComponent("lockbook", isDirectory: true)

        if !fm.fileExists(atPath: newURL.path) && hasContents(at: legacyURL) {
            do {
                try copyLegacyDataThenDeleteContents(from: legacyURL, to: newURL)
            } catch {
                NSLog("Lockbook migration failed copying %@ to %@: %@", legacyURL.path, newURL.path, "\(error)")
                return legacyURL.path
            }
        }
        try? fm.createDirectory(at: newURL, withIntermediateDirectories: true)

        return newURL.path
    }

    private static func copyLegacyDataThenDeleteContents(from legacyURL: URL, to newURL: URL) throws {
        let fm = FileManager.default

        try fm.copyItem(at: legacyURL, to: newURL)

        for legacyItem in try fm.contentsOfDirectory(at: legacyURL, includingPropertiesForKeys: nil) {
            try! fm.removeItem(at: legacyItem)
        }
    }

    private static func hasContents(at url: URL) -> Bool {
        guard let contents = try? FileManager.default.contentsOfDirectory(atPath: url.path) else {
            return false
        }

        return !contents.isEmpty
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
