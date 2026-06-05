import SwiftUI
import SwiftWorkspace

public enum SearchMode: String, CaseIterable, Identifiable, Hashable {
    case path = "Filename"
    case content = "Content"
    public var id: String { rawValue }
    
    static var platformDefault: SearchMode {
        .path
    }
}

struct SearchContainerSubView<Content: View>: View {
    @EnvironmentObject var workspaceInput: WorkspaceInputState
    @EnvironmentObject var homeState: HomeState
    
    @Binding var isSearching: Bool
    @ObservedObject var model: SearchContainerViewModel
    let dismissSearch: () -> Void
    
    let content: Content
    
    private func openAndCloseFloatingSidebar(id: UUID, match: ContentSearcherMatch? = nil) {
        model.open(id: id, workspaceInput: workspaceInput, match: match)
        if homeState.isSidebarFloating {
            homeState.sidebarState = .closed
        }
    }
    
    var body: some View {
        Group {
            if isSearching {
                VStack(spacing: 0) {
                    modePicker
                    if model.isQuerying {
                        querySpinner
                    } else if model.mode == .content, let focused = model.focusedResult {
                        FocusedSearchResultView(
                            result: focused,
                            fetchSnippet: { match in
                                model.snippet(id: focused.id, match: match)
                            },
                            onBack: { model.focusedResult = nil },
                            onTapSnippet: { match in
                                openAndCloseFloatingSidebar(id: focused.id, match: match)
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
    
    var querySpinner: some View {
        ProgressView()
            .frame(maxWidth: .infinity, maxHeight: .infinity)
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
                        onTap: { openAndCloseFloatingSidebar(id: result.id, match: result.matches.first) },
                        onShowMore: { model.focusedResult = result }
                    )
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

class SearchContainerViewModel: ObservableObject {
    @Published var input: String = ""
    @Published var isShown: Bool = false
    @Published var mode: SearchMode = .platformDefault
    @Published var contentResults: [ContentSearcherResult] = []
    @Published var pathResults: [PathSearcherResult] = []
    @Published var focusedResult: ContentSearcherResult? = nil
    @Published var isQuerying: Bool = false
    
    let filesModel: FilesViewModel
    
    private var contentSearcher: ContentSearching?
    private var pathSearcher: PathSearching?
    
    private let searchQueue = DispatchQueue(label: "lockbook.search")
    private var querySeq: UInt64 = 0
    
    init(filesModel: FilesViewModel) {
        self.filesModel = filesModel
    }

    func startSearching() {
        guard contentSearcher == nil else { return }
        contentSearcher = AppState.lb.contentSearcher()
        pathSearcher = AppState.lb.pathSearcher()
        search()
    }

    func stopSearching() {
        querySeq &+= 1
        contentSearcher = nil
        pathSearcher = nil
        contentResults = []
        pathResults = []
        focusedResult = nil
        isQuerying = false
    }
    
    func search() {
        querySeq &+= 1
        let seq = querySeq
        let mode = mode
        let input = input
        let contentSearcher = contentSearcher
        let pathSearcher = pathSearcher
        
        guard contentSearcher != nil || pathSearcher != nil else { return }
        
        focusedResult = nil
        isQuerying = true

        searchQueue.async { [weak self] in
            var content: [ContentSearcherResult] = []
            var path: [PathSearcherResult] = []
            switch mode {
            case .content: content = contentSearcher?.query(input) ?? []
            case .path: path = pathSearcher?.query(input) ?? []
            }

            DispatchQueue.main.async {
                guard let self, self.querySeq == seq else { return }
                switch mode {
                case .content: self.contentResults = content
                case .path: self.pathResults = path
                }
                self.isQuerying = false
            }
        }
    }
    
    func snippet(id: UUID, match: ContentSearcherMatch) -> SearcherSnippet? {
        guard let contentSearcher else { return nil }
        return searchQueue.sync { contentSearcher.snippet(id: id, match: match, contextChars: 40) }
    }
    
    func open(id: UUID, workspaceInput: WorkspaceInputState, match: ContentSearcherMatch? = nil) {
        guard let file = filesModel.idsToFiles[id] else { return }
        if file.type == .folder {
            workspaceInput.selectFolder(id: id)
        } else if let match {
            workspaceInput.openFile(id: id, rangeStart: match.rangeStart, rangeEnd: match.rangeEnd)
        } else {
            workspaceInput.openFile(id: id)
        }
    }
}
