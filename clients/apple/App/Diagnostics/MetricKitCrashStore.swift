import CryptoKit
import Foundation

/// One immutable file per diagnostic, consumed by lb-rs alongside Rust panic logs.
/// Call on a serial queue. File publication is atomic so sync never reads a partial report.
struct MetricKitCrashStore {
    struct Diagnostic {
        let applicationVersion: String
        let json: Data
    }

    let directory: () -> URL

    func save(_ crashes: [Diagnostic], begin: Date, end: Date, bundleIdentifier: String) throws {
        guard !crashes.isEmpty else { return }

        let directory = directory()
        let fm = FileManager.default
        try fm.createDirectory(at: directory, withIntermediateDirectories: true)
        let formatter = ISO8601DateFormatter()
        formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        var occurrences: [Data: Int] = [:]

        for crash in crashes {
            var report: [String: Any] = [
                "source": "MetricKit",
                "bundleIdentifier": bundleIdentifier,
                "applicationVersion": crash.applicationVersion,
                // MetricKit supplies a reporting interval, not an exact crash timestamp.
                "timeStampBegin": formatter.string(from: begin),
                "timeStampEnd": formatter.string(from: end),
                "diagnostic": try JSONSerialization.jsonObject(with: crash.json),
            ]
            let canonical = try JSONSerialization.data(withJSONObject: report, options: [.sortedKeys])
            // Preserve repeated identical crashes within a payload, but not redelivery
            // of that payload (including pastDiagnosticPayloads on subsequent launches).
            report["occurrence"] = occurrences[canonical, default: 0]
            occurrences[canonical, default: 0] += 1
            let json = try JSONSerialization.data(withJSONObject: report, options: [.sortedKeys, .prettyPrinted])
            let id = SHA256.hash(data: json).map { String(format: "%02x", $0) }.joined()
            let file = directory.appendingPathComponent("metrickit---\(id).log")
            guard !fm.fileExists(atPath: file.path) else { continue }

            let header = "INFO: MetricKit crash in \(bundleIdentifier) (version \(crash.applicationVersion))\n"
            try (Data(header.utf8) + json).write(to: file, options: .atomic)
        }
    }
}
