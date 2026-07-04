import CoreTransferable
import Foundation
import SwiftUI
import SwiftWorkspace
import UniformTypeIdentifiers

enum FileDropItem: Transferable {
    case file(File)
    case url(URL)

    static var transferRepresentation: some TransferRepresentation {
        ProxyRepresentation { (file: File) in FileDropItem.file(file) }
        ProxyRepresentation { (url: URL) in FileDropItem.url(url) }
    }
}

extension View {
    @ViewBuilder
    func fileDropTarget(
        isTargeted: @escaping (Bool) -> Void = { _ in },
        action: @escaping ([FileDropItem]) -> Bool
    ) -> some View {
        #if os(macOS)
            onDrop(
                of: [.lockbookFile, .fileURL],
                delegate: MacFileDropDelegate(targeted: isTargeted, action: action)
            )
        #else
            dropDestination(for: FileDropItem.self) { items, _ in
                action(items)
            } isTargeted: { targeted in
                isTargeted(targeted)
            }
        #endif
    }
}

#if os(macOS)
    struct MacFileDropDelegate: DropDelegate {
        let targeted: (Bool) -> Void
        let action: ([FileDropItem]) -> Bool

        func validateDrop(info: DropInfo) -> Bool {
            info.hasItemsConforming(to: [.lockbookFile, .fileURL])
        }

        func dropEntered(info _: DropInfo) {
            targeted(true)
        }

        func dropExited(info _: DropInfo) {
            targeted(false)
        }

        func dropUpdated(info: DropInfo) -> DropProposal? {
            DropProposal(operation: info.hasItemsConforming(to: [.lockbookFile]) ? .move : .copy)
        }

        func performDrop(info _: DropInfo) -> Bool {
            targeted(false)

            let items = Self.readDragPasteboard()

            guard !items.isEmpty else {
                return false
            }

            return action(items)
        }

        private static func readDragPasteboard() -> [FileDropItem] {
            let pasteboard = NSPasteboard(name: .drag)
            let metadataType = NSPasteboard.PasteboardType(UTType.lockbookFile.identifier)

            var items: [FileDropItem] = []

            for item in pasteboard.pasteboardItems ?? [] {
                if let data = item.data(forType: metadataType),
                   let file = try? JSONDecoder().decode(File.self, from: data)
                {
                    items.append(.file(file))
                } else if let raw = item.string(forType: .fileURL),
                          let url = URL(string: raw), url.isFileURL
                {
                    items.append(.url(url.standardizedFileURL))
                }
            }

            return items
        }
    }
#endif

#if os(iOS)
struct DraggedFile: Transferable {
    let file: File
    let filesModel: FilesModel

    static var transferRepresentation: some TransferRepresentation {
        FileRepresentation(exportedContentType: .item) { dragged in
            try await SentTransferredFile(
                dragged.export(), allowAccessingOriginalFile: false
            )
        }

        ProxyRepresentation(exporting: \.file)
    }

    private func export() async throws -> URL {
        await MainActor.run {
            filesModel.exportsInProgress += 1
        }

        defer {
            Task { @MainActor in
                filesModel.exportsInProgress -= 1
            }
        }

        return try Self.exportToTemp(file)
    }

    private static func exportToTemp(_ file: File) throws -> URL {
        let dir = URL(fileURLWithPath: NSTemporaryDirectory())
            .appendingPathComponent("lb-export")
            .appendingPathComponent(UUID().uuidString)

        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)

        if case let .failure(err) = AppState.lb.exportFile(
            sourceId: file.id, dest: dir.path(), edit: false
        ) {
            throw err
        }

        return dir.appendingPathComponent(file.name)
    }
}
#endif
