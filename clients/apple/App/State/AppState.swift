import Foundation
import Observation
import SwiftWorkspace

#if os(macOS)
    import AppKit
#else
    import UIKit
#endif

@Observable class AppState {
    static let shared = AppState()

    var account: Account? = nil
    var isLoggedIn: Bool = false
    var error: UIError? = nil

    private init() {
        checkIfLoggedIn()

        #if os(macOS)
            let foregroundNotification = NSApplication.didBecomeActiveNotification
        #else
            let foregroundNotification = UIApplication.didBecomeActiveNotification
        #endif

        NotificationCenter.default.addObserver(
            forName: foregroundNotification,
            object: nil,
            queue: .main
        ) { _ in
            AppState.lb.appForegrounded()
        }
    }

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
            try fm.removeItem(at: legacyItem)
        }
    }

    private static func hasContents(at url: URL) -> Bool {
        guard let contents = try? FileManager.default.contentsOfDirectory(atPath: url.path) else {
            return false
        }

        return !contents.isEmpty
    }
    #endif

    static let defaultApiUrl: String =
        ProcessInfo.processInfo.environment["API_LOCATION"] ?? "https://app.lockbook.net"

    static let lb: LbAPI = {
        if isPreviewEnvironmentKey.defaultValue {
            return MockLb()
        }

        return Lb(writablePath: ProcessInfo.processInfo.environment["LOCKBOOK_PATH"] ?? LB_LOC, logs: true)
    }()

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
