import Foundation
import MetricKit

/// Compatibility for systems before iOS/macOS 27. Never subscribed alongside
/// MetricManager; remove this adapter when the app's minimum OS reaches 27.
final class LegacyMetricKitCrashReporter: NSObject, MXMetricManagerSubscriber {
    private let queue = DispatchQueue(label: "app.lockbook.metrickit")
    private let store: MetricKitCrashStore

    init(store: MetricKitCrashStore) {
        self.store = store
        super.init()
    }

    func start() {
        MXMetricManager.shared.add(self)
        // Recover reports if the app exited before a previous delivery was saved.
        didReceive(MXMetricManager.shared.pastDiagnosticPayloads)
    }

    deinit {
        MXMetricManager.shared.remove(self)
    }

    func didReceive(_ payloads: [MXDiagnosticPayload]) {
        queue.async { [self] in
            for payload in payloads {
                let crashes = (payload.crashDiagnostics ?? []).map {
                    MetricKitCrashStore.Diagnostic(
                        applicationVersion: $0.applicationVersion,
                        json: $0.jsonRepresentation()
                    )
                }
                do {
                    try store.save(
                        crashes,
                        begin: payload.timeStampBegin,
                        end: payload.timeStampEnd,
                        bundleIdentifier: Bundle.main.bundleIdentifier ?? "app.lockbook"
                    )
                } catch {
                    NSLog("Could not save MetricKit crash diagnostics: %@", String(describing: error))
                }
            }
        }
    }
}
