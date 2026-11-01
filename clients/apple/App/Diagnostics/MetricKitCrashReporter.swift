import Darwin
import Foundation
import MetricKit

/// Keep collection independent of Lb initialization; Rust handles upload consent.
final class MetricKitCrashReporter {
    private let directory: () -> URL
    private var task: Task<Void, Never>?

    init(directory: @escaping () -> URL) {
        self.directory = directory
    }

    func start() {
        guard #available(iOS 27.0, macOS 27.0, *), task == nil else { return }

        // One manager and consumer for the app's lifetime. Persist serially off
        // the main actor, without retaining this reporter through the task.
        task = Task.detached(priority: .utility) { [directory] in
            let manager = MetricManager()
            defer { withExtendedLifetime(manager) {} }
            for await report in manager.diagnosticReports {
                guard !Task.isCancelled else { break }
                do {
                    try Self.save(report, directory: directory)
                } catch {
                    NSLog("Could not save MetricKit crash diagnostics: %@", String(describing: error))
                }
            }
        }
    }

    deinit {
        task?.cancel()
    }

    @available(iOS 27.0, macOS 27.0, *)
    private static func save(_ report: DiagnosticReport, directory: () -> URL) throws {
        guard case .crash = report.result else { return }

        let encoder = JSONEncoder()
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
        let json = try encoder.encode(report)

        // Use receipt time so the existing panic-file uploader notices delayed
        // reports. The JSON retains Apple's diagnostic timing and crashed build.
        let formatter = DateFormatter()
        formatter.locale = Locale(identifier: "en_US_POSIX")
        formatter.dateFormat = "yyyy-MM-dd'---'HH-mm-ss"
        let directory = directory()
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        let file = directory.appendingPathComponent("panic---\(formatter.string(from: Date())).log")

        // Append like the Android/Rust writer, preserving reports in the same second.
        let descriptor = Darwin.open(file.path, O_WRONLY | O_CREAT | O_APPEND, S_IRUSR | S_IWUSR)
        guard descriptor != -1 else {
            throw NSError(domain: NSPOSIXErrorDomain, code: Int(errno))
        }
        let handle = FileHandle(fileDescriptor: descriptor, closeOnDealloc: true)
        defer { try? handle.close() }

        let environment = report.environment
        let header = "INFO: MetricKit crash in \(environment.bundleIdentifier) (version \(environment.applicationVersion))\nBT: "
        try handle.write(contentsOf: Data(header.utf8) + json + Data("\n".utf8))
    }
}
