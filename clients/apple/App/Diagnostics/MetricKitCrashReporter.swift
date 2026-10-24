import Foundation
import MetricKit

/// Keep collection independent of Lb initialization; Rust handles upload consent.
final class MetricKitCrashReporter {
    private let store: MetricKitCrashStore
    private var task: Task<Void, Never>?
    private var legacyReporter: LegacyMetricKitCrashReporter?

    init(directory: @escaping () -> URL) {
        store = MetricKitCrashStore(directory: directory)
    }

    func start() {
        guard task == nil, legacyReporter == nil else { return }

        if #available(iOS 27.0, macOS 27.0, *) {
            // One manager and consumer for the app's lifetime. Persist serially off
            // the main actor, without retaining this reporter through the task.
            task = Task.detached(priority: .utility) { [store] in
                let manager = MetricManager()
                defer { withExtendedLifetime(manager) {} }
                for await report in manager.diagnosticReports {
                    guard !Task.isCancelled else { break }
                    do {
                        try store.save(report)
                    } catch {
                        NSLog("Could not save MetricKit crash diagnostics: %@", String(describing: error))
                    }
                }
            }
        } else {
            // The app still supports OS 26; the Swift API requires OS 27.
            let reporter = LegacyMetricKitCrashReporter(store: store)
            legacyReporter = reporter
            reporter.start()
        }
    }

    deinit {
        task?.cancel()
    }
}
