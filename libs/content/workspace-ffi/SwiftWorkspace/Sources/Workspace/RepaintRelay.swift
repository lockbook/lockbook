import Foundation

enum RepaintRelay {
    private static let lock = NSLock()
    private static var handlers: [UnsafeMutableRawPointer: (UInt64) -> Void] = [:]

    static func register(
        _ key: UnsafeMutableRawPointer, _ handler: @escaping (UInt64) -> Void
    ) {
        lock.lock()
        handlers[key] = handler
        lock.unlock()
    }

    static func unregister(_ key: UnsafeMutableRawPointer) {
        lock.lock()
        handlers.removeValue(forKey: key)
        lock.unlock()
    }

    static func fire(_ key: UnsafeMutableRawPointer?, _ delayMs: UInt64) {
        guard let key else { return }

        lock.lock()
        let handler = handlers[key]
        lock.unlock()

        handler?(delayMs)
    }
}
