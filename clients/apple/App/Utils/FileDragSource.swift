import CoreTransferable
import Foundation
import SwiftUI
import SwiftWorkspace
import UniformTypeIdentifiers

#if os(macOS)
    import AppKit
#endif

enum FileExporter {
    static func exportToTemp(_ file: File, edit: Bool, filesModel: FilesModel?) throws -> URL {
        DispatchQueue.main.async { filesModel?.exportsInProgress += 1 }

        defer {
            DispatchQueue.main.async { filesModel?.exportsInProgress -= 1 }
        }

        do {
            let dir = URL(fileURLWithPath: NSTemporaryDirectory())
                .appendingPathComponent("lb-export")
                .appendingPathComponent(UUID().uuidString)

            try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)

            if case let .failure(err) = AppState.lb.exportFile(
                sourceId: file.id, dest: dir.path(percentEncoded: false), edit: edit
            ) {
                throw err
            }

            return dir.appendingPathComponent(file.name)
        } catch {
            report(error)
            throw error
        }
    }

    static func report(_ error: Error) {
        DispatchQueue.main.async {
            if let lbError = error as? LbError {
                AppState.shared.error = .lb(error: lbError)
            } else {
                AppState.shared.error = .custom(
                    title: "Export failed", msg: error.localizedDescription
                )
            }
        }
    }
}

struct DraggedFile: Transferable {
    let file: File
    let filesModel: FilesModel

    static var transferRepresentation: some TransferRepresentation {
        FileRepresentation(exportedContentType: .item) { dragged in
            SentTransferredFile(
                try FileExporter.exportToTemp(
                    dragged.file, edit: false, filesModel: dragged.filesModel
                ),
                allowAccessingOriginalFile: false
            )
        }

        ProxyRepresentation(exporting: \.file)
    }
}

#if os(macOS)
    struct MacFileDragSource: NSViewRepresentable {
        let file: File
        let filesModel: FilesModel
        let onClick: () -> Void
        let onDoubleClick: () -> Void

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
            view.onDoubleClick = onDoubleClick
        }
    }

    final class FileDragSourceView: NSView, NSDraggingSource, NSFilePromiseProviderDelegate {
        var file: File? = nil
        var filesModel: FilesModel? = nil
        var onClick: (() -> Void)? = nil
        var onDoubleClick: (() -> Void)? = nil

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

            if event.clickCount == 2 {
                onDoubleClick?()
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

            let exported: URL
            do {
                exported = try FileExporter.exportToTemp(file, edit: true, filesModel: filesModel)
            } catch {
                completionHandler(error)
                return
            }

            do {
                try FileManager.default.moveItem(at: exported, to: url)
                completionHandler(nil)
            } catch {
                FileExporter.report(error)
                completionHandler(error)
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
