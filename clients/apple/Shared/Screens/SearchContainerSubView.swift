import SwiftUI
import SwiftWorkspace

public enum SearchMode: String, CaseIterable, Identifiable, Hashable {
    case path = "Filename"
    case content = "Content"
    public var id: String { rawValue }

    static var platformDefault: SearchMode {
        #if os(iOS)
            .path
        #else
            .content
        #endif
    }
}

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
                    modePicker

                    if model.mode == .content, let focused = model.focusedResult {
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
                        switch model.mode {
                        case .content:
                            contentResultsList
                        case .path:
                            pathResultsList
                        }
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
        .onChange(of: model.mode) { _ in
            model.search()
        }
    }

    var modePicker: some View {
        Picker("", selection: $model.mode) {
            ForEach(SearchMode.allCases) { m in
                Text(m.rawValue).tag(m)
            }
        }
        .pickerStyle(.segmented)
        .padding(.horizontal, 10)
        .padding(.vertical, 6)
    }

    var contentResultsList: some View {
        ScrollView {
            LazyVStack(alignment: .leading, spacing: 0) {
                ForEach(model.contentResults) { result in
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

    var pathResultsList: some View {
        ScrollView {
            LazyVStack(alignment: .leading, spacing: 0) {
                ForEach(model.pathResults) { result in
                    PathSearcherRow(
                        result: result,
                        onTap: { openAndCloseFloatingSidebar(id: result.id) }
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

struct PathSearcherRow: View {
    let result: PathSearcherResult
    let onTap: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            highlighted(result.filename, offset: filenameOffset)
                .font(.body)
            if !result.parentPath.isEmpty, result.parentPath != "/" {
                highlighted(result.parentPath, offset: parentOffset)
                    .font(.caption)
                    .foregroundColor(.secondary)
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(.horizontal, 12)
        .padding(.vertical, 8)
        .contentShape(Rectangle())
        .onTapGesture { onTap() }
    }

    private var parentOffset: Int {
        // leading "/" consumes index 0, so parent text starts at 1 for nested paths
        result.parentPath == "/" ? 0 : 1
    }

    private var filenameOffset: Int {
        // root: "/" + filename → filename starts at 1
        // nested: "/" + parent + "/" + filename → starts at parent.count + 2
        if result.parentPath.isEmpty || result.parentPath == "/" {
            return 1
        }
        return result.parentPath.unicodeScalars.count + 2
    }

    private func highlighted(_ s: String, offset: Int) -> Text {
        let indices = Set(result.matchedIndices.map { Int($0) })
        var out = Text("")
        for (i, scalar) in s.unicodeScalars.enumerated() {
            let part = Text(String(scalar))
            if indices.contains(i + offset) {
                out = out + part.bold().foregroundColor(.primary)
            } else {
                out = out + part.foregroundColor(.secondary)
            }
        }
        return out
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
            metric(label: "results", value: "\(model.resultCount)")
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
    @Published var mode: SearchMode = .platformDefault
    @Published var buildDuration: TimeInterval? = nil
    @Published var lastQueryDuration: TimeInterval? = nil
    @Published var contentResults: [ContentSearcherResult] = []
    @Published var pathResults: [PathSearcherResult] = []
    @Published var rendered: Set<UUID> = []
    @Published var focusedResult: ContentSearcherResult? = nil

    let filesModel: FilesViewModel

    private var contentSearcher: ContentSearching?
    private var pathSearcher: PathSearching?

    init(filesModel: FilesViewModel) {
        self.filesModel = filesModel
    }

    var resultCount: Int {
        switch mode {
        case .content: contentResults.count
        case .path: pathResults.count
        }
    }

    func startSearching() {
        guard contentSearcher == nil else { return }
        let start = Date()
        contentSearcher = AppState.lb.contentSearcher()
        pathSearcher = AppState.lb.pathSearcher()
        buildDuration = Date().timeIntervalSince(start)
        search()
    }

    func stopSearching() {
        contentSearcher = nil
        pathSearcher = nil
        buildDuration = nil
        lastQueryDuration = nil
        contentResults = []
        pathResults = []
        rendered = []
        focusedResult = nil
    }

    func search() {
        let start = Date()
        switch mode {
        case .content:
            if let contentSearcher {
                contentResults = contentSearcher.query(input)
            }
        case .path:
            if let pathSearcher {
                pathResults = pathSearcher.query(input)
            }
        }
        lastQueryDuration = Date().timeIntervalSince(start)
        focusedResult = nil
        rendered = []
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
