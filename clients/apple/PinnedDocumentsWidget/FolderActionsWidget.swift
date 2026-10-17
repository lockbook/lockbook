import AppIntents
import Bridge
import SwiftUI
import WidgetKit

struct WidgetFolder: AppEntity {
    let id: UUID
    let name: String
    let path: String

    static var typeDisplayRepresentation: TypeDisplayRepresentation = "Location"
    static var defaultQuery = WidgetFolderQuery()

    var displayRepresentation: DisplayRepresentation {
        DisplayRepresentation(title: "\(name)", subtitle: "\(path)", image: .init(systemName: "folder"))
    }

    static let preview = WidgetFolder(id: UUID(), name: "Notes", path: "/Notes/")
}

struct WidgetFolderQuery: EntityStringQuery {
    func entities(for identifiers: [UUID]) async throws -> [WidgetFolder] {
        let folders = loadFolders()
        return identifiers.map { id in
            // Preserve a deleted folder's identity so it never silently becomes Home.
            folders.first { $0.id == id } ?? WidgetFolder(id: id, name: "Location unavailable", path: "")
        }
    }

    func suggestedEntities() async throws -> [WidgetFolder] {
        loadFolders()
    }

    func entities(matching string: String) async throws -> [WidgetFolder] {
        loadFolders().filter {
            $0.name.localizedCaseInsensitiveContains(string) || $0.path.localizedCaseInsensitiveContains(string)
        }
    }

    func defaultResult() async -> WidgetFolder? {
        loadFolders().first { $0.path == "/" }
    }

    private func loadFolders() -> [WidgetFolder] {
        (try? withWidgetCore { core in
            let result = lb_list_metadatas(core)
            defer { lb_free_file_list_res(result) }
            guard result.err == nil else { return [] }

            return UnsafeBufferPointer(start: result.list.list, count: Int(result.list.count))
                .filter { $0.typ.tag == LbFolder }
                .compactMap { file in
                    let path = lb_get_path_by_id(core, file.id)
                    defer { lb_free_path_res(path) }
                    guard path.err == nil else { return nil }
                    let id = UUID(uuid: file.id.bytes)
                    return WidgetFolder(
                        id: id,
                        name: id == UUID(uuid: file.parent.bytes) ? "Home" : String(cString: file.name),
                        path: String(cString: path.path)
                    )
                }
                .sorted { $0.path.localizedStandardCompare($1.path) == .orderedAscending }
        }) ?? []
    }
}

struct FolderActionsConfiguration: WidgetConfigurationIntent {
    static var title: LocalizedStringResource = "Choose Location"
    static var description = IntentDescription("Choose where to create notes and drawings.")

    @Parameter(title: "Location") var folder: WidgetFolder?
}

struct FolderActionsEntry: TimelineEntry {
    let date: Date
    let folder: WidgetFolder?
}

struct FolderActionsProvider: AppIntentTimelineProvider {
    func placeholder(in context: Context) -> FolderActionsEntry {
        FolderActionsEntry(date: .now, folder: .preview)
    }

    func snapshot(for configuration: FolderActionsConfiguration, in context: Context) async -> FolderActionsEntry {
        if context.isPreview { return placeholder(in: context) }
        return await entry(for: configuration)
    }

    func timeline(for configuration: FolderActionsConfiguration, in context: Context) async -> Timeline<FolderActionsEntry> {
        let entry = await entry(for: configuration)
        return Timeline(entries: [entry], policy: .after(.now.addingTimeInterval(15 * 60)))
    }

    private func entry(for configuration: FolderActionsConfiguration) async -> FolderActionsEntry {
        let query = WidgetFolderQuery()
        let folder: WidgetFolder?
        if let selected = configuration.folder {
            folder = try? await query.entities(for: [selected.id]).first
        } else {
            folder = await query.defaultResult()
        }
        return FolderActionsEntry(date: .now, folder: folder)
    }
}

struct FolderActionsWidgetView: View {
    @Environment(\.widgetFamily) private var family
    let entry: FolderActionsEntry

    var body: some View {
        Group {
            if let folder = entry.folder {
                if family == .systemSmall {
                    Button(intent: CreateDocumentIntent(folder: folder)) {
                        VStack(alignment: .leading, spacing: 8) {
                            Text("New note")
                                .font(.caption.weight(.semibold))
                            Image(systemName: "plus")
                                .font(.title2.weight(.medium))
                                .foregroundStyle(Color.accentColor)
                                .frame(width: 48, height: 48)
                                .background(Color.accentColor.opacity(0.15), in: Circle())
                                .widgetAccentable()
                                .frame(maxWidth: .infinity, maxHeight: .infinity)
                            Label(folder.name, systemImage: "folder")
                                .font(.caption)
                                .foregroundStyle(.secondary)
                                .lineLimit(1)
                        }
                        .accessibilityElement(children: .ignore)
                        .accessibilityLabel("New note in \(folder.name)")
                    }
                    .buttonStyle(.plain)
                } else {
                    VStack(alignment: .leading, spacing: 8) {
                        HStack {
                            Label(folder.name, systemImage: "folder")
                                .font(.caption.weight(.semibold))
                                .lineLimit(1)
                            Spacer(minLength: 4)
                            Text("Lockbook")
                                .font(.caption2)
                                .foregroundStyle(.secondary)
                        }
                        HStack(spacing: 8) {
                            Button(intent: CreateDocumentIntent(folder: folder)) {
                                actionLabel("Note", icon: "doc.badge.plus")
                            }
                            Button(intent: CreateDocumentIntent(folder: folder, drawing: true)) {
                                actionLabel("Drawing", icon: "pencil.and.outline")
                            }
                            Link(destination: URL(string: "lb://search")!) {
                                actionLabel("Search", icon: "magnifyingglass")
                            }
                        }
                        .buttonStyle(.plain)
                    }
                }
            } else {
                Text("Open Lockbook to set up your account, then choose a location in the widget's settings.")
                    .font(.callout)
                    .foregroundStyle(.secondary)
                    .widgetURL(URL(string: "lb://search")!)
            }
        }
        .privacySensitive()
        .containerBackground(.background, for: .widget)
    }

    private func actionLabel(_ title: String, icon: String) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            Image(systemName: icon)
                .font(.title3)
                .foregroundStyle(Color.accentColor)
                .widgetAccentable()
            Spacer(minLength: 0)
            Text(title)
                .font(.caption)
                .foregroundStyle(.primary)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .leading)
        .padding(10)
        .background(.quaternary.opacity(0.6), in: RoundedRectangle(cornerRadius: 9))
        .contentShape(RoundedRectangle(cornerRadius: 9))
    }
}

struct FolderActionsWidget: Widget {
    var body: some WidgetConfiguration {
        AppIntentConfiguration(kind: "FolderActions", intent: FolderActionsConfiguration.self,
                               provider: FolderActionsProvider()) { entry in
            FolderActionsWidgetView(entry: entry)
        }
        .configurationDisplayName("Quick Create")
        .description("Create notes and drawings in a folder, or jump into search.")
        .supportedFamilies([.systemSmall, .systemMedium])
    }
}

#Preview(as: .systemSmall) {
    FolderActionsWidget()
} timeline: {
    FolderActionsEntry(date: .now, folder: .preview)
}

#Preview(as: .systemMedium) {
    FolderActionsWidget()
} timeline: {
    FolderActionsEntry(date: .now, folder: .preview)
}
