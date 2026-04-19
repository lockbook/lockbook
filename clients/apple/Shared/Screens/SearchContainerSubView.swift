import SwiftUI
import SwiftWorkspace

struct SearchContainerSubView<Content: View>: View {
    @EnvironmentObject var workspaceInput: WorkspaceInputState
    @EnvironmentObject var homeState: HomeState

    @Binding var isSearching: Bool
    @ObservedObject var model: SearchContainerViewModel
    let dismissSearch: () -> Void

    let content: Content

    private func openAndCloseFloatingSidebar(id: UUID) {
        model.open(id: id, workspaceInput: workspaceInput)
        if homeState.isSidebarFloating {
            homeState.sidebarState = .closed
        }
    }

    var body: some View {
        Group {
            if isSearching {
                VStack(spacing: 0) {
                    if let focused = model.focusedResult {
                        FocusedSearchResultView(
                            result: focused,
                            fetchSnippet: { match in
                                model.snippet(id: focused.id, match: match)
                            },
                            onBack: { model.focusedResult = nil },
                            onTapSnippet: { _ in
                                openAndCloseFloatingSidebar(id: focused.id)
                            }
                        )
                    } else {
                        resultsList
                    }
                    SearchMetricsBar(model: model)
                }
            } else {
                content
            }
        }
        .onChange(of: isSearching) { newValue in
            if newValue {
                model.startSearching()
            } else {
                model.stopSearching()
            }
        }
    }

    var resultsList: some View {
        ScrollView {
            LazyVStack(alignment: .leading, spacing: 0) {
                ForEach(model.results) { result in
                    SearchResultRow(
                        result: result,
                        fetchSnippet: { match in model.snippet(id: result.id, match: match) },
                        onTap: { openAndCloseFloatingSidebar(id: result.id) },
                        onShowMore: { model.focusedResult = result }
                    )
                    .onAppear { model.rendered.insert(result.id) }
                    .onDisappear { model.rendered.remove(result.id) }
                    Divider()
                }
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

struct SearchResultRow: View {
    let result: ContentSearcherResult
    let fetchSnippet: (ContentSearcherMatch) -> SearcherSnippet?
    let onTap: () -> Void
    let onShowMore: () -> Void

    private static let collapsedCount = 2

    var visibleMatches: ArraySlice<ContentSearcherMatch> {
        result.matches.prefix(Self.collapsedCount)
    }

    var hiddenCount: Int {
        max(0, result.matches.count - Self.collapsedCount)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(result.filename)
                .font(.body)
            Text(result.parentPath)
                .font(.caption)
                .foregroundColor(.secondary)

            ForEach(Array(visibleMatches.enumerated()), id: \.offset) { _, match in
                snippetLine(for: match)
            }

            if hiddenCount > 0 {
                Button(action: onShowMore) {
                    Text("Show \(hiddenCount) more")
                        .font(.caption)
                        .foregroundColor(.accentColor)
                }
                .buttonStyle(.plain)
                .padding(.top, 2)
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(.horizontal, 12)
        .padding(.vertical, 8)
        .contentShape(Rectangle())
        .onTapGesture { onTap() }
    }

    @ViewBuilder
    func snippetLine(for match: ContentSearcherMatch) -> some View {
        if let snippet = fetchSnippet(match) {
            (Text(snippet.prefix).foregroundColor(.gray)
                + Text(snippet.matched).bold()
                + Text(snippet.suffix).foregroundColor(.gray))
                .font(.caption)
                .lineLimit(1)
                .truncationMode(.tail)
                .frame(maxWidth: .infinity, alignment: .leading)
        }
    }
}

struct FocusedSearchResultView: View {
    let result: ContentSearcherResult
    let fetchSnippet: (ContentSearcherMatch) -> SearcherSnippet?
    let onBack: () -> Void
    let onTapSnippet: (ContentSearcherMatch) -> Void

    var body: some View {
        VStack(spacing: 0) {
            header
            Divider()
            snippetList
        }
    }

    var header: some View {
        HStack(spacing: 8) {
            Button(action: onBack) {
                Image(systemName: "chevron.left")
                    .foregroundColor(.accentColor)
            }
            .buttonStyle(.plain)

            VStack(alignment: .leading, spacing: 2) {
                Text(result.filename)
                    .font(.headline)
                Text(result.parentPath)
                    .font(.caption)
                    .foregroundColor(.secondary)
            }

            Spacer()

            Text("\(result.matches.count) match\(result.matches.count == 1 ? "" : "es")")
                .font(.caption)
                .foregroundColor(.secondary)
        }
        .padding()
    }

    var snippetList: some View {
        ScrollView {
            LazyVStack(alignment: .leading, spacing: 0) {
                ForEach(Array(result.matches.enumerated()), id: \.offset) { _, match in
                    Button(action: { onTapSnippet(match) }) {
                        snippetRow(for: match)
                    }
                    .buttonStyle(.plain)
                    Divider()
                }
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    @ViewBuilder
    func snippetRow(for match: ContentSearcherMatch) -> some View {
        if let snippet = fetchSnippet(match) {
            (Text(snippet.prefix).foregroundColor(.gray)
                + Text(snippet.matched).bold()
                + Text(snippet.suffix).foregroundColor(.gray))
                .font(.caption)
                .lineLimit(1)
                .truncationMode(.tail)
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(.horizontal, 12)
                .padding(.vertical, 8)
                .contentShape(Rectangle())
        }
    }
}

struct SearchMetricsBar: View {
    @ObservedObject var model: SearchContainerViewModel

    var body: some View {
        HStack(spacing: 12) {
            if let dur = model.buildDuration {
                metric(label: "build", value: format(ms: dur * 1000))
            }
            if let dur = model.lastQueryDuration {
                metric(label: "query", value: format(ms: dur * 1000))
            }
            metric(label: "results", value: "\(model.results.count)")
            metric(label: "rendered", value: "\(model.rendered.count)")
            Spacer()
        }
        .font(.system(.caption2, design: .monospaced))
        .padding(.horizontal, 10)
        .padding(.vertical, 4)
        .background(Color.gray.opacity(0.08))
    }

    func metric(label: String, value: String) -> some View {
        HStack(spacing: 4) {
            Text(label).foregroundColor(.gray.opacity(0.7))
            Text(value).foregroundColor(.gray)
        }
    }

    func format(ms: Double) -> String {
        ms < 10 ? String(format: "%.2f ms", ms) : String(format: "%.0f ms", ms)
    }
}

class SearchContainerViewModel: ObservableObject {
    @Published var input: String = ""
    @Published var isShown: Bool = false
    @Published var buildDuration: TimeInterval? = nil
    @Published var lastQueryDuration: TimeInterval? = nil
    @Published var results: [ContentSearcherResult] = []
    @Published var rendered: Set<UUID> = []
    @Published var focusedResult: ContentSearcherResult? = nil

    let filesModel: FilesViewModel

    private var contentSearcher: ContentSearching?

    init(filesModel: FilesViewModel) {
        self.filesModel = filesModel
    }

    func startSearching() {
        guard contentSearcher == nil else { return }
        let start = Date()
        contentSearcher = AppState.lb.contentSearcher()
        buildDuration = Date().timeIntervalSince(start)
    }

    func stopSearching() {
        contentSearcher = nil
        buildDuration = nil
        lastQueryDuration = nil
        results = []
        rendered = []
        focusedResult = nil
    }

    func search() {
        guard let contentSearcher else { return }
        let start = Date()
        results = contentSearcher.query(input)
        lastQueryDuration = Date().timeIntervalSince(start)
        focusedResult = nil
    }

    func snippet(id: UUID, match: ContentSearcherMatch) -> SearcherSnippet? {
        contentSearcher?.snippet(id: id, match: match, contextChars: 40)
    }

    func open(id: UUID, workspaceInput: WorkspaceInputState) {
        guard let file = filesModel.idsToFiles[id] else { return }
        if file.type == .folder {
            workspaceInput.selectFolder(id: id)
        } else {
            workspaceInput.openFile(id: id)
        }
    }
}
