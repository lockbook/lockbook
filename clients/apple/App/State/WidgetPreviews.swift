#if os(iOS)
    import Bridge
    import Foundation
    import SwiftWorkspace
    import UIKit
    import WidgetKit

    /// Renders the pinned documents for the widget, which can't run the
    /// renderer itself, into the shared container: light and dark, filed by
    /// id beside the version they show. A document renders again only when
    /// its version changes.
    @MainActor final class WidgetPreviews {
        static let shared = WidgetPreviews()

        /// Each preview's size in points; tiles show them top-aligned and clipped.
        static let size = CGSize(width: 200, height: 200)

        /// The page as it reads on this device, the preview's width in its points.
        private static var page: CGFloat {
            min(UIScreen.main.bounds.width, UIScreen.main.bounds.height)
        }

        private var rendering = false
        private var queued: (WorkspaceInputState, [UUID])?
        /// Set once the app is in the background, where the GPU refuses work.
        private var stop = Flag()
        /// The background time the current render holds, if any.
        private var task = UIBackgroundTaskIdentifier.invalid

        private init() {
            NotificationCenter.default.addObserver(
                forName: UIApplication.didEnterBackgroundNotification, object: nil, queue: .main
            ) { [weak self] _ in self?.stop.set() }
        }

        /// Renders the pins whose previews are missing or behind their
        /// document, drops the previews of documents no longer pinned, and
        /// reloads the widget when anything changed.
        func refresh(workspace: WorkspaceInputState, pins: [UUID]) {
            guard !rendering else {
                queued = (workspace, pins)
                return
            }
            guard let directory = WidgetPreviewFiles.directory else { return }
            try? FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
            WidgetPreviewFiles.prune(keeping: Set(pins))

            var jobs: [(UUID, Bool, PreviewJob)] = []
            var versions: [UUID: String] = [:]
            for id in pins {
                guard let version = Self.version(of: id),
                      version != WidgetPreviewFiles.version(of: id) else { continue }
                for dark in [false, true] {
                    guard let job = workspace.previewDocument(
                        id: id,
                        size: CGSize(width: Self.page, height: Self.page * Self.size.height / Self.size.width),
                        scale: UIScreen.main.scale * Self.size.width / Self.page,
                        dark: dark
                    ) else { continue }
                    jobs.append((id, dark, job))
                }
                versions[id] = version
            }
            guard !jobs.isEmpty else { return }

            rendering = true
            let stop = Flag()
            self.stop = stop
            // iOS ends an app that holds background time past its expiry
            task = UIApplication.shared.beginBackgroundTask(withName: "Widget previews") {
                stop.set()
                MainActor.assumeIsolated { self.endTask() }
            }
            DispatchQueue.global(qos: .utility).async {
                var rendered: [UUID: Int] = [:]
                for (id, dark, job) in jobs {
                    // a job not rendered is freed by rendering it; the GPU can't once stopped
                    guard !stop.isSet else {
                        WorkspaceInputState.discardPreview(job)
                        continue
                    }
                    guard let png = WorkspaceInputState.renderPreview(job) else { continue }
                    try? png.write(to: WidgetPreviewFiles.image(of: id, dark: dark), options: .atomic)
                    rendered[id, default: 0] += 1
                }
                // a version stands for both appearances
                for (id, count) in rendered where count == 2 {
                    if let version = versions[id] {
                        WidgetPreviewFiles.setVersion(version, of: id)
                    }
                }
                DispatchQueue.main.async {
                    self.rendering = false
                    WidgetCenter.shared.reloadAllTimelines()
                    self.endTask()
                    if let (workspace, pins) = self.queued {
                        self.queued = nil
                        self.refresh(workspace: workspace, pins: pins)
                    }
                }
            }
        }

        private func endTask() {
            guard task != .invalid else { return }
            UIApplication.shared.endBackgroundTask(task)
            task = .invalid
        }

        /// A flag set from any thread.
        final class Flag: @unchecked Sendable {
            private let lock = NSLock()
            private var value = false

            var isSet: Bool { lock.withLock { value } }

            func set() { lock.withLock { value = true } }
        }

        /// The document's content version as hex, from core without reading it.
        static func version(of id: UUID) -> String? {
            guard let lb = AppState.lb.lbUnsafeRawPtr else { return nil }
            let result = lb_document_version(OpaquePointer(lb), LbUuid(bytes: id.uuid))
            if let err = result.err {
                lb_free_err(err)
                return nil
            }
            guard result.version.present else { return "new" }
            var hmac = result.version.hmac
            return withUnsafeBytes(of: &hmac) { $0.map { String(format: "%02x", $0) }.joined() }
        }
    }
#endif
