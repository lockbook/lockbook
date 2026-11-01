import Bridge
import SwiftUI
import UIKit
import WidgetKit

struct PinnedDocument: Identifiable {
    let id: UUID
    let name: String
    /// The document as the app last drew it, in each appearance, when that
    /// drawing shows its current content.
    var light: UIImage? = nil
    var dark: UIImage? = nil

    var url: URL { URL(string: "lb://\(id.uuidString)")! }

    func preview(_ scheme: ColorScheme) -> UIImage? {
        scheme == .dark ? dark ?? light : light ?? dark
    }
}

struct PinnedDocumentsEntry: TimelineEntry {
    let date: Date
    let documents: [PinnedDocument]
    var message: String? = nil

    static let preview = PinnedDocumentsEntry(date: .now, documents: [
        PinnedDocument(id: UUID(), name: "Daily notes.md"),
        PinnedDocument(id: UUID(), name: "Weekend plans.md"),
        PinnedDocument(id: UUID(), name: "Reading list.md"),
        PinnedDocument(id: UUID(), name: "Travel.pdf"),
        PinnedDocument(id: UUID(), name: "Ideas.md"),
        PinnedDocument(id: UUID(), name: "Sketch.svg"),
    ])
}

struct PinnedDocumentsProvider: TimelineProvider {
    func placeholder(in context: Context) -> PinnedDocumentsEntry {
        .preview
    }

    func getSnapshot(in context: Context, completion: @escaping (PinnedDocumentsEntry) -> Void) {
        if context.isPreview {
            completion(.preview)
        } else {
            load(completion: completion)
        }
    }

    func getTimeline(in context: Context, completion: @escaping (Timeline<PinnedDocumentsEntry>) -> Void) {
        load { entry in
            completion(Timeline(entries: [entry], policy: .after(.now.addingTimeInterval(15 * 60))))
        }
    }

    private func load(completion: @escaping (PinnedDocumentsEntry) -> Void) {
        DispatchQueue.global(qos: .userInitiated).async {
            guard let directory = FileManager.default.containerURL(
                forSecurityApplicationGroupIdentifier: "group.app.lockbook"
            )?.appendingPathComponent("lockbook", isDirectory: true) else {
                completion(PinnedDocumentsEntry(
                    date: .now, documents: [], message: "Open Lockbook to set up your account."
                ))
                return
            }

            let core = lb_init(directory.path, "widgets", BuildInfo.identity, true, false)
            defer { lb_free_init(core) }

            guard core.err == nil else {
                completion(PinnedDocumentsEntry(
                    date: .now, documents: [], message: "Open Lockbook to load your pinned documents."
                ))
                return
            }

            let pins = lb_list_pinned(core.lb)
            defer { lb_free_id_list_res(pins) }

            guard pins.err == nil else {
                completion(PinnedDocumentsEntry(
                    date: .now, documents: [], message: "Open Lockbook to load your pinned documents."
                ))
                return
            }

            let documents = UnsafeBufferPointer(start: pins.ids, count: Int(pins.len))
                .compactMap { id -> PinnedDocument? in
                    let result = lb_get_file(core.lb, id)
                    defer { lb_free_file_res(result) }
                    guard result.err == nil else { return nil }
                    let id = UUID(uuid: result.file.id.bytes)
                    var document = PinnedDocument(id: id, name: String(cString: result.file.name))
                    if WidgetPreviewFiles.version(of: id) == version(core.lb, id) {
                        document.light = UIImage(contentsOfFile: WidgetPreviewFiles.image(of: id, dark: false).path)
                        document.dark = UIImage(contentsOfFile: WidgetPreviewFiles.image(of: id, dark: true).path)
                    }
                    return document
                }
                .sorted {
                    let comparison = $0.name.localizedStandardCompare($1.name)
                    return comparison == .orderedSame ? $0.id < $1.id : comparison == .orderedAscending
                }
            completion(PinnedDocumentsEntry(date: .now, documents: documents))
        }
    }
}

/// A document's content version as hex, as the app files its previews.
private func version(_ lb: OpaquePointer?, _ id: UUID) -> String? {
    let result = lb_document_version(lb, LbUuid(bytes: id.uuid))
    if let err = result.err {
        lb_free_err(err)
        return nil
    }
    guard result.version.present else { return "new" }
    var hmac = result.version.hmac
    return withUnsafeBytes(of: &hmac) { $0.map { String(format: "%02x", $0) }.joined() }
}

struct PinnedDocumentsWidgetView: View {
    @Environment(\.widgetFamily) private var family
    @Environment(\.colorScheme) private var colorScheme
    let entry: PinnedDocumentsEntry

    private var limit: Int {
        switch family {
        case .systemSmall: 1
        case .systemLarge: 6
        default: 3
        }
    }

    private var documents: [PinnedDocument] { Array(entry.documents.prefix(limit)) }

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                Label("Pinned", systemImage: "pin.fill")
                    .font(.caption.weight(.semibold))
                Spacer()
                if family != .systemSmall {
                    Text("Lockbook")
                        .font(.caption2)
                        .foregroundStyle(.secondary)
                }
            }

            if entry.documents.isEmpty {
                Text(entry.message ?? "Pin documents in Lockbook to keep them here.")
                    .font(.callout)
                    .foregroundStyle(.secondary)
                    .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .leading)
            } else if family == .systemSmall, let document = documents.first {
                documentTile(document)
                    .widgetURL(document.url)
            } else {
                documentGrid
                if family == .systemLarge, entry.documents.count > limit {
                    Text("+\(entry.documents.count - limit) more in Lockbook")
                        .font(.caption2)
                        .foregroundStyle(.secondary)
                }
            }
        }
        .privacySensitive()
        .containerBackground(.background, for: .widget)
    }

    private var documentGrid: some View {
        let columns = family == .systemLarge ? 2 : 3
        let rows = family == .systemLarge ? 3 : 1

        return VStack(spacing: 8) {
            ForEach(0..<rows, id: \.self) { row in
                HStack(spacing: 8) {
                    ForEach(0..<columns, id: \.self) { column in
                        let index = row * columns + column
                        if index < documents.count {
                            let document = documents[index]
                            Link(destination: document.url) {
                                documentTile(document)
                                    .background(.quaternary.opacity(0.6), in: RoundedRectangle(cornerRadius: WidgetPreviewFiles.cornerRadius))
                                    .clipShape(RoundedRectangle(cornerRadius: WidgetPreviewFiles.cornerRadius))
                                    .contentShape(RoundedRectangle(cornerRadius: WidgetPreviewFiles.cornerRadius))
                            }
                            .buttonStyle(.plain)
                        } else {
                            Color.clear
                        }
                    }
                }
            }
        }
    }

    /// The document's preview, top-aligned and clipped, over its name; its
    /// icon and name until the app has drawn it.
    @ViewBuilder private func documentTile(_ document: PinnedDocument) -> some View {
        if let preview = document.preview(colorScheme) {
            VStack(alignment: .leading, spacing: 0) {
                Color.clear
                    .overlay(alignment: .top) {
                        Image(uiImage: preview)
                            .resizable()
                            .scaledToFill()
                    }
                    .clipped()
                Text(document.name)
                    .font(.caption2)
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
                    .padding(.horizontal, family == .systemSmall ? 0 : 8)
                    .padding(.vertical, 4)
            }
            .accessibilityElement(children: .ignore)
            .accessibilityLabel(document.name)
        } else {
            documentLabel(document)
                .padding(family == .systemSmall ? 0 : 10)
        }
    }

    private func documentLabel(_ document: PinnedDocument) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            Image(systemName: FileIconHelper.docNameToSystemImageName(name: document.name))
                .font(family == .systemSmall ? .title : .title3)
                .foregroundStyle(Color.accentColor)
                .widgetAccentable()
            Spacer(minLength: 0)
            Text(document.name)
                .font(family == .systemSmall ? .body : .caption)
                .foregroundStyle(.primary)
                .lineLimit(2)
                .multilineTextAlignment(.leading)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .leading)
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(document.name)
    }
}

@main
struct LockbookWidgets: WidgetBundle {
    var body: some Widget {
        PinnedDocumentsWidget()
        FolderActionsWidget()
    }
}

struct PinnedDocumentsWidget: Widget {
    var body: some WidgetConfiguration {
        StaticConfiguration(kind: "PinnedDocuments", provider: PinnedDocumentsProvider()) { entry in
            PinnedDocumentsWidgetView(entry: entry)
        }
        .configurationDisplayName("Pinned Documents")
        .description("Open your pinned Lockbook documents.")
        .supportedFamilies([.systemSmall, .systemMedium, .systemLarge])
    }
}

#Preview(as: .systemMedium) {
    PinnedDocumentsWidget()
} timeline: {
    PinnedDocumentsEntry.preview
    PinnedDocumentsEntry(date: .now, documents: Array(PinnedDocumentsEntry.preview.documents.prefix(1)))
    PinnedDocumentsEntry(date: .now, documents: Array(PinnedDocumentsEntry.preview.documents.prefix(2)))
    PinnedDocumentsEntry(date: .now, documents: [])
}

#Preview(as: .systemSmall) {
    PinnedDocumentsWidget()
} timeline: {
    PinnedDocumentsEntry.preview
    PinnedDocumentsEntry(date: .now, documents: [])
}

#Preview(as: .systemLarge) {
    PinnedDocumentsWidget()
} timeline: {
    PinnedDocumentsEntry.preview
    PinnedDocumentsEntry(date: .now, documents: Array(PinnedDocumentsEntry.preview.documents.prefix(1)))
    PinnedDocumentsEntry(date: .now, documents: [])
}
