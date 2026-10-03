import Bridge
import SwiftUI
import WidgetKit

struct PinnedDocument: Identifiable {
    let id: UUID
    let name: String

    var url: URL { URL(string: "lb://\(id.uuidString)")! }
}

struct PinnedDocumentsEntry: TimelineEntry {
    let date: Date
    let documents: [PinnedDocument]
    var message: String? = nil

    static let preview = PinnedDocumentsEntry(date: .now, documents: [
        PinnedDocument(id: UUID(), name: "Daily notes.md"),
        PinnedDocument(id: UUID(), name: "Weekend plans.md"),
        PinnedDocument(id: UUID(), name: "Reading list.md"),
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

            let result = lb_widget_pinned_documents(directory.path)
            defer { lb_free_file_list_res(result) }

            guard result.err == nil else {
                completion(PinnedDocumentsEntry(
                    date: .now, documents: [], message: "Open Lockbook to load your pinned documents."
                ))
                return
            }

            let documents = UnsafeBufferPointer(start: result.list.list, count: Int(result.list.count))
                .map { file in
                    PinnedDocument(id: UUID(uuid: file.id.bytes), name: String(cString: file.name))
                }
                .sorted {
                    let comparison = $0.name.localizedStandardCompare($1.name)
                    return comparison == .orderedSame ? $0.id < $1.id : comparison == .orderedAscending
                }
            completion(PinnedDocumentsEntry(date: .now, documents: documents))
        }
    }
}

struct PinnedDocumentsWidgetView: View {
    @Environment(\.widgetFamily) private var family
    let entry: PinnedDocumentsEntry

    private var limit: Int { family == .systemLarge ? 8 : 3 }

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                Label("Pinned", systemImage: "pin.fill")
                    .font(.headline)
                Spacer()
                Text("Lockbook")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }

            if entry.documents.isEmpty {
                Text(entry.message ?? "Pin documents in Lockbook to keep them here.")
                    .font(.callout)
                    .foregroundStyle(.secondary)
                    .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .leading)
            } else {
                ForEach(entry.documents.prefix(limit)) { document in
                    Link(destination: document.url) {
                        Label(document.name, systemImage: "doc.text")
                            .font(.callout)
                            .lineLimit(1)
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                }
                Spacer(minLength: 0)
                if entry.documents.count > limit {
                    Text("+\(entry.documents.count - limit) more in Lockbook")
                        .font(.caption2)
                        .foregroundStyle(.secondary)
                }
            }
        }
        .privacySensitive()
        .containerBackground(.background, for: .widget)
    }
}

@main
struct PinnedDocumentsWidget: Widget {
    var body: some WidgetConfiguration {
        StaticConfiguration(kind: "PinnedDocuments", provider: PinnedDocumentsProvider()) { entry in
            PinnedDocumentsWidgetView(entry: entry)
        }
        .configurationDisplayName("Pinned Documents")
        .description("Open your pinned Lockbook documents.")
        .supportedFamilies([.systemMedium, .systemLarge])
    }
}

#Preview(as: .systemMedium) {
    PinnedDocumentsWidget()
} timeline: {
    PinnedDocumentsEntry.preview
    PinnedDocumentsEntry(date: .now, documents: [])
}
