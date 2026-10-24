import Foundation
import MetricKit

@available(iOS 27.0, macOS 27.0, *)
extension MetricKitCrashStore {
    func save(_ report: DiagnosticReport) throws {
        guard case .crash = report.result else { return }

        // Keep the entire typed report, including the crashed build's environment
        // and binary information, rather than using the currently running build.
        let crash = Diagnostic(
            applicationVersion: report.environment.applicationVersion,
            json: try JSONEncoder().encode(report)
        )
        try save(
            [crash],
            begin: report.timeRange.start,
            end: report.timeRange.end,
            bundleIdentifier: report.environment.bundleIdentifier
        )
    }
}
