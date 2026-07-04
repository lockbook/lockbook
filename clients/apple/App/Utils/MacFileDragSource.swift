#if os(macOS)
    import AppKit
    import SwiftUI
    import SwiftWorkspace
    import UniformTypeIdentifiers

    struct MacFileDragSource: NSViewRepresentable {
        let file: File
        let filesModel: FilesModel
        let onClick: () -> Void

        func makeNSView(context _: Context) -> FileDragSourceView {
            let view = FileDragSourceView()
            update(view)
            return view
        }

        func updateNSView(_ view: FileDragSourceView, context _: Context) {
            update(view)
        }

        private func update(_ view: FileDragSourceView) {
            view.file = file
            view.filesModel = filesModel
            view.onClick = onClick
        }
    }

    final class FileDragSourceView: NSView, NSDraggingSource, NSFilePromiseProviderDelegate {
        var file: File? = nil
        var filesModel: FilesModel? = nil
        var onClick: (() -> Void)? = nil

        static let exportQueue: OperationQueue = {
            let queue = OperationQueue()
            queue.qualityOfService = .userInitiated
            return queue
        }()

        override func acceptsFirstMouse(for _: NSEvent?) -> Bool {
            true
        }

        override func mouseDown(with event: NSEvent) {
            if event.modifierFlags.contains(.control) {
                super.mouseDown(with: event)
                return
            }

            let start = event.locationInWindow

            while let next = window?.nextEvent(matching: [.leftMouseDragged, .leftMouseUp]) {
                if next.type == .leftMouseUp {
                    onClick?()
                    return
                }

                let dx = next.locationInWindow.x - start.x
                let dy = next.locationInWindow.y - start.y

                if dx * dx + dy * dy > 9 {
                    beginDrag(with: next)
                    return
                }
            }
        }

        private func beginDrag(with event: NSEvent) {
            guard let file else {
                return
            }

            let fileType: UTType = if file.isFolder {
                .folder
            } else {
                file.name.split(separator: ".").last
                    .flatMap { UTType(filenameExtension: String($0)) } ?? .data
            }

            let provider = LbFilePromiseProvider(fileType: fileType.identifier, delegate: self)
            provider.userInfo = file
            provider.metadata = try? JSONEncoder().encode(file)

            let location = convert(event.locationInWindow, from: nil)
            let frame = NSRect(x: location.x - 16, y: location.y - 16, width: 32, height: 32)

            let item = NSDraggingItem(pasteboardWriter: provider)
            item.setDraggingFrame(frame, contents: NSWorkspace.shared.icon(for: fileType))

            beginDraggingSession(with: [item], event: event, source: self)
        }

        func draggingSession(
            _: NSDraggingSession, sourceOperationMaskFor context: NSDraggingContext
        ) -> NSDragOperation {
            context == .withinApplication ? [.move, .copy, .generic] : .copy
        }

        func filePromiseProvider(
            _ provider: NSFilePromiseProvider, fileNameForType _: String
        ) -> String {
            (provider.userInfo as? File)?.name ?? "untitled"
        }

        func filePromiseProvider(
            _ provider: NSFilePromiseProvider,
            writePromiseTo url: URL,
            completionHandler: @escaping (Error?) -> Void
        ) {
            guard let file = provider.userInfo as? File else {
                completionHandler(NSError(
                    domain: "net.lockbook",
                    code: -1,
                    userInfo: [NSLocalizedDescriptionKey: "Drag-out lost file context"]
                ))
                return
            }

            let filesModel = filesModel
            DispatchQueue.main.async { filesModel?.exportsInProgress += 1 }

            let error = Self.export(file, to: url)

            DispatchQueue.main.async {
                filesModel?.exportsInProgress -= 1

                if let error {
                    if let lbError = error as? LbError {
                        AppState.shared.error = .lb(error: lbError)
                    } else {
                        AppState.shared.error = .custom(
                            title: "Export failed", msg: error.localizedDescription
                        )
                    }
                }
            }

            completionHandler(error)
        }

        private static func export(_ file: File, to url: URL) -> Error? {
            let scratch = URL(fileURLWithPath: NSTemporaryDirectory())
                .appendingPathComponent("lb-export")
                .appendingPathComponent(UUID().uuidString)

            do {
                try FileManager.default.createDirectory(at: scratch, withIntermediateDirectories: true)

                if case let .failure(err) = AppState.lb.exportFile(
                    sourceId: file.id, dest: scratch.path(percentEncoded: false), edit: true
                ) {
                    return err
                }

                try FileManager.default.moveItem(
                    at: scratch.appendingPathComponent(file.name), to: url
                )

                return nil
            } catch {
                return error
            }
        }

        func operationQueue(for _: NSFilePromiseProvider) -> OperationQueue {
            Self.exportQueue
        }
    }

    final class LbFilePromiseProvider: NSFilePromiseProvider {
        var metadata: Data? = nil

        override func writableTypes(for pasteboard: NSPasteboard) -> [NSPasteboard.PasteboardType] {
            var types = super.writableTypes(for: pasteboard)

            if metadata != nil {
                types.append(NSPasteboard.PasteboardType(UTType.lockbookFile.identifier))
            }

            return types
        }

        override func pasteboardPropertyList(forType type: NSPasteboard.PasteboardType) -> Any? {
            if type.rawValue == UTType.lockbookFile.identifier {
                return metadata
            }

            return super.pasteboardPropertyList(forType: type)
        }

        override func writingOptions(
            forType type: NSPasteboard.PasteboardType, pasteboard: NSPasteboard
        ) -> NSPasteboard.WritingOptions {
            if type.rawValue == UTType.lockbookFile.identifier {
                return []
            }

            return super.writingOptions(forType: type, pasteboard: pasteboard)
        }
    }
#endif
