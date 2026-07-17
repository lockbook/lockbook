import SwiftUI
import SwiftWorkspace

struct RecentsView: View {
    @Environment(FilesModel.self) private var filesModel
    @Environment(WorkspaceInputState.self) private var workspaceInput
    #if os(iOS)
        @Environment(HomeState.self) private var homeState
    #endif

    let model: RecentsModel
    let fileTreeModel: FileTreeModel

    var recentDocs: [File] {
        filesModel.idsToFiles.values
            .filter { $0.type == .document }
            .sorted {
                if $0.lastModified != $1.lastModified {
                    return $0.lastModified > $1.lastModified
                }

                return $0.id < $1.id
            }
    }

    var body: some View {
        let docs = recentDocs
        let orderedIds = docs.map(\.id)
        let sections = Self.sections(for: docs)

        Group {
            if docs.isEmpty {
                EmptyStateView(
                    title: "No documents yet",
                    subtitle: "Documents you edit will appear here."
                )
            } else {
                ScrollView {
                    LazyVStack(alignment: .leading, spacing: 0) {
                        ForEach(sections, id: \.title) { section in
                            Text(section.title)
                                .font(.title3.weight(.semibold))
                                .padding(.horizontal, 12)
                                .padding(.top, 22)
                                .padding(.bottom, 6)

                            ForEach(section.files) { file in
                                VStack(spacing: 0) {
                                    RecentDocRow(file: file, model: model, orderedIds: orderedIds)

                                    if file.id != section.files.last?.id {
                                        Divider()
                                            .padding(.leading, 58)
                                    }
                                }
                            }
                        }
                    }
                    .padding(.bottom, 20)
                }
                .refreshable {
                    #if os(iOS)
                        homeState.explicitSyncRequested()
                    #endif
                    workspaceInput.requestSync()
                }
            }
        }
        .fileDropTarget { items in
            guard let root = filesModel.root else {
                return false
            }

            return filesModel.drop(items, into: root)
        }
        .selectionCommands(model.selection)
        .environment(fileTreeModel)
        .navigationTitle("Recents")
        .largeNavigationTitle()
    }

    private static func sections(for docs: [File]) -> [(title: String, files: [File])] {
        let calendar = Calendar.current
        let startOfToday = calendar.startOfDay(for: .now)

        var sections: [(title: String, files: [File])] = []

        for file in docs {
            let modified = Date(timeIntervalSince1970: TimeInterval(file.lastModified) / 1000)
            let title = sectionTitle(for: modified, calendar: calendar, startOfToday: startOfToday)

            if sections.last?.title == title {
                sections[sections.count - 1].files.append(file)
            } else {
                sections.append((title: title, files: [file]))
            }
        }

        return sections
    }

    private static func sectionTitle(for date: Date, calendar: Calendar, startOfToday: Date) -> String {
        if date >= startOfToday {
            return "Today"
        }

        if let yesterday = calendar.date(byAdding: .day, value: -1, to: startOfToday), date >= yesterday {
            return "Yesterday"
        }

        if let weekAgo = calendar.date(byAdding: .day, value: -7, to: startOfToday), date >= weekAgo {
            return "Previous 7 Days"
        }

        if let monthAgo = calendar.date(byAdding: .day, value: -30, to: startOfToday), date >= monthAgo {
            return "Previous 30 Days"
        }

        if calendar.component(.year, from: date) == calendar.component(.year, from: startOfToday) {
            return date.formatted(.dateTime.month(.wide))
        }

        return date.formatted(.dateTime.year())
    }

}

struct RecentDocRow: View {
    @Environment(FilesModel.self) private var filesModel
    @Environment(HomeState.self) private var homeState
    @Environment(WorkspaceInputState.self) private var workspaceInput

    let file: File
    let model: RecentsModel
    let orderedIds: [UUID]

    var body: some View {
        HStack(alignment: .top, spacing: 12) {
            SelectionIndicator(selection: model.selection, id: file.id)
                .padding(.top, 1)

            Image(systemName: FileIconHelper.docNameToSystemImageName(name: file.name))
                .font(.title3)
                .foregroundColor(.accentColor)
                .frame(width: 22)
                .padding(.top, 1)

            VStack(alignment: .leading, spacing: 6) {
                HStack(alignment: .firstTextBaseline) {
                    Text(file.name)
                        .fontWeight(.medium)
                        .lineLimit(1)
                        .truncationMode(.tail)

                    if filesModel.pinnedIds.contains(file.id) {
                        Image(systemName: "pin.fill")
                            .font(.system(size: 10))
                            .foregroundStyle(.orange)
                    }

                    Spacer(minLength: 8)

                    Text(min(modifiedDate, .now), format: .relative(presentation: .named))
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }

                FileBreadcrumb(file: file)

                if let snippet {
                    Text(snippet)
                        .font(.subheadline)
                        .foregroundStyle(.secondary)
                        .lineLimit(2)
                }

                if let username = AppState.shared.account?.username, file.lastModifiedBy != username {
                    Label(file.lastModifiedBy, systemImage: "person.fill")
                        .font(.caption.weight(.medium))
                        .foregroundStyle(Color.accentColor)
                        .padding(.horizontal, 8)
                        .padding(.vertical, 3)
                        .background(Capsule().fill(Color.accentColor.opacity(0.15)))
                        .padding(.top, 2)
                }
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(.horizontal, 12)
        .padding(.vertical, 14)
        .contentShape(Rectangle())
        .selectableRow(
            model.selection,
            id: file.id,
            orderedIds: { orderedIds },
            open: {
                workspaceInput.openFile(id: file.id)
                homeState.compactColumn = .detail
            }
        )
        .onAppear {
            model.loadPreviewIfNeeded(file)
        }
        .onChange(of: file.lastModified) {
            model.loadPreviewIfNeeded(file)
        }
    }

    private var modifiedDate: Date {
        Date(timeIntervalSince1970: TimeInterval(file.lastModified) / 1000)
    }

    private var snippet: AttributedString? {
        if file.name.split(separator: ".").last?.lowercased() == "svg" {
            return AttributedString("Drawing")
        }

        guard let preview = model.preview(for: file) else {
            return nil
        }

        return preview.characters.isEmpty ? AttributedString("No additional text") : preview
    }

}

#Preview {
    NavigationStack {
        RecentsView(model: RecentsModel(), fileTreeModel: .preview)
    }
    .environment(FilesModel.preview)
    .environment(HomeState())
    .environment(WorkspaceInputState.preview)
}
