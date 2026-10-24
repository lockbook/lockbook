import Foundation
import MetricKit

/// MetricKit delivers diagnostics from previous runs on a background queue.
/// Keep collection independent of Lb initialization; Rust handles upload consent.
final class MetricKitCrashReporter: NSObject, MXMetricManagerSubscriber {
    private let queue = DispatchQueue(label: "app.lockbook.metrickit")
    private let store: MetricKitCrashStore

    init(directory: @escaping () -> URL) {
        store = MetricKitCrashStore(directory: directory)
        super.init()
    }

    func start() {
        MXMetricManager.shared.add(self)
        // Recover reports if the app exited before a previous delivery was saved.
        didReceive(MXMetricManager.shared.pastDiagnosticPayloads)
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
