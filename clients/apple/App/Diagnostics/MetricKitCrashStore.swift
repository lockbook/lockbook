import CryptoKit
import Foundation
import MetricKit

/// One immutable file per diagnostic, consumed by lb-rs alongside Rust panic logs.
/// Serialize calls from a single consumer. Atomic publication prevents partial reads by sync.
struct MetricKitCrashStore {
    let directory: () -> URL

    func save(_ report: DiagnosticReport) throws {
        guard case .crash = report.result else { return }

        // Preserve the crashed build's environment and binary information. Stable
        // encoding gives redeliveries the same filename without rewriting the file.
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys, .prettyPrinted]
        let json = try encoder.encode(report)
        let id = SHA256.hash(data: json).map { String(format: "%02x", $0) }.joined()

        let directory = directory()
        let fm = FileManager.default
        try fm.createDirectory(at: directory, withIntermediateDirectories: true)
        let file = directory.appendingPathComponent("metrickit---\(id).log")
        guard !fm.fileExists(atPath: file.path) else { return }

        let environment = report.environment
        let header = "INFO: MetricKit crash in \(environment.bundleIdentifier) (version \(environment.applicationVersion))\n"
        try (Data(header.utf8) + json).write(to: file, options: .atomic)
    }
}
