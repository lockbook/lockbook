import Foundation
import Observation
import SwiftWorkspace

#if os(macOS)
    import AppKit
#else
    import UIKit
    import UIKit.UIGestureRecognizerSubclass
#endif

@Observable class AppState {
    static let shared = AppState()

    var account: Account? = nil
    var isLoggedIn: Bool = false
    var error: UIError? = nil
    var accountCreatedThisSession = false

    @ObservationIgnored private var lastActivity = Date.distantPast

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

        #if os(macOS)
            NSEvent.addLocalMonitorForEvents(matching: [.leftMouseDown, .rightMouseDown, .scrollWheel, .keyDown]) { [weak self] event in
                self?.userActive()
                return event
            }
        #else
            NotificationCenter.default.addObserver(
                forName: UIWindow.didBecomeKeyNotification,
                object: nil,
                queue: .main
            ) { [weak self] notification in
                self?.watch(notification.object as? UIWindow)
            }
            for case let scene as UIWindowScene in UIApplication.shared.connectedScenes {
                scene.windows.forEach { watch($0) }
            }
        #endif
    }

    #if os(iOS)
        private func watch(_ window: UIWindow?) {
            guard let window, window.gestureRecognizers?.contains(where: { $0 is ActivityRecognizer }) != true else {
                return
            }

            window.addGestureRecognizer(ActivityRecognizer { [weak self] in self?.userActive() })
        }
    #endif

    private func userActive() {
        guard Date().timeIntervalSince(lastActivity) >= 1 else {
            return
        }

        lastActivity = Date()
        AppState.lb.appForegrounded()
    }

    static let LB_LOC: String = {
        #if os(macOS) && DEBUG
            NSHomeDirectory() + "/.lockbook"
        #elseif os(macOS)
            do {
                let fm = FileManager.default
                guard let group = fm.containerURL(forSecurityApplicationGroupIdentifier: "group.app.lockbook") else {
                    throw CocoaError(.fileWriteNoPermission)
                }
                let location = group.appendingPathComponent("data", isDirectory: true)
                let legacy = URL(fileURLWithPath: NSHomeDirectory()).appendingPathComponent(".lockbook", isDirectory: true)
                if !fm.fileExists(atPath: location.path), fm.fileExists(atPath: legacy.path) {
                    try fm.moveItem(at: legacy, to: location)
                }
                try fm.createDirectory(at: location, withIntermediateDirectories: true)
                return location.path
            } catch {
                fatalError("Could not move Lockbook data to its shared container: \(error)")
            }
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

        #if os(macOS)
        let clientName = "macos"
        #else
        let clientName = "ios"
        #endif
        let core = Lb(writablePath: ProcessInfo.processInfo.environment["LOCKBOOK_PATH"] ?? LB_LOC, clientName: clientName, buildInfo: BuildInfo.identity, logs: true)
        return core
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

#if os(iOS)
    final class ActivityRecognizer: UIGestureRecognizer {
        private let onActivity: () -> Void

        init(onActivity: @escaping () -> Void) {
            self.onActivity = onActivity
            super.init(target: nil, action: nil)
            cancelsTouchesInView = false
        }

        override func touchesBegan(_: Set<UITouch>, with _: UIEvent) {
            onActivity()
            state = .failed
        }

        override func pressesBegan(_: Set<UIPress>, with _: UIPressesEvent) {
            onActivity()
            state = .failed
        }
    }
#endif
