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

struct SearchTabView: View {
    @EnvironmentObject var workspaceInput: WorkspaceInputState
    @EnvironmentObject var homeState: HomeState

    @StateObject private var model: SearchViewModel
    @FocusState private var fieldFocused: Bool

    /// One-shot request (e.g. from a ⌘⇧F shortcut) to open in a specific mode; consumed on apply.
    @Binding private var requestedMode: SearchMode?

    init(filesModel: FilesViewModel, requestedMode: Binding<SearchMode?> = .constant(nil)) {
        _model = StateObject(wrappedValue: SearchViewModel(filesModel: filesModel))
        _requestedMode = requestedMode
    }

    var body: some View {
        VStack(spacing: 0) {
            searchField
            modePicker
            Divider()
            results
        }
        .navigationTitle("Search")
        #if os(iOS)
            .navigationBarTitleDisplayMode(.inline)
            .onKeyPress(.upArrow) { moveSelection(-1) }
            .onKeyPress(.downArrow) { moveSelection(1) }
            .onKeyPress(.return) { openSelectedViaKey() }
        #endif
        .onAppear {
            model.refresh()
            applyRequestedMode()
            fieldFocused = true
        }
        .onChange(of: requestedMode) { _ in applyRequestedMode() }
        .onChange(of: model.input) { _ in model.search() }
        .onChange(of: model.mode) { _ in model.search() }
    }

    /// Apply and clear a pending mode request, focusing the field.
    private func applyRequestedMode() {
        guard let requestedMode else { return }
        model.mode = requestedMode
        fieldFocused = true
        self.requestedMode = nil
    }

    private func open(_ id: UUID, match: ContentSearcherMatch? = nil) {
        model.open(id: id, workspaceInput: workspaceInput, match: match)
        if homeState.isSidebarFloating {
            homeState.sidebarState = .closed
        }
    }

    #if os(iOS)
        /// Arrow-key handler: only steals the key when a result list is showing.
        private func moveSelection(_ delta: Int) -> KeyPress.Result {
            guard model.focusedResult == nil, model.resultCount > 0 else {
                return .ignored
            }
            model.moveSelection(delta)
            return .handled
        }

        private func openSelectedViaKey() -> KeyPress.Result {
            guard model.focusedResult == nil, model.resultCount > 0 else {
                return .ignored
            }
            openSelected()
            return .handled
        }
    #endif

    /// Open the keyboard-highlighted result (Return key / tap).
    private func openSelected() {
        switch model.mode {
        case .content:
            guard model.contentResults.indices.contains(model.selected) else { return }
            let result = model.contentResults[model.selected]
            open(result.id, match: result.matches.first)
        case .path:
            guard model.pathResults.indices.contains(model.selected) else { return }
            open(model.pathResults[model.selected].id)
        }
    }

    var searchField: some View {
        HStack(spacing: 8) {
            Image(systemName: "magnifyingglass")
                .foregroundColor(.secondary)
            TextField("Search", text: $model.input)
                .textFieldStyle(.plain)
                .focused($fieldFocused)
                .submitLabel(.search)
                .onSubmit { fieldFocused = false }
                #if os(iOS)
                    .textInputAutocapitalization(.never)
                #endif
                .autocorrectionDisabled()
            if !model.input.isEmpty {
                Button(action: { model.input = "" }) {
                    Image(systemName: "xmark.circle.fill")
                        .foregroundColor(.secondary)
                }
                .buttonStyle(.plain)
            }
        }
        .padding(8)
        .background(RoundedRectangle(cornerRadius: 10).fill(Color.gray.opacity(0.15)))
        .padding(.horizontal)
        .padding(.vertical, 8)
    }

    var modePicker: some View {
        Picker("", selection: $model.mode) {
            ForEach(SearchMode.allCases) { mode in
                Text(mode.rawValue).tag(mode)
            }
        }
        .pickerStyle(.segmented)
        .padding(.horizontal)
        .padding(.bottom, 6)
    }

    @ViewBuilder
    var results: some View {
        if model.isQuerying {
            ProgressView()
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        } else if model.mode == .content, let focused = model.focusedResult {
            FocusedSearchResultView(
                result: focused,
                systemImage: model.icon(for: focused.id, name: focused.filename),
                fetchSnippet: { match in model.snippet(id: focused.id, match: match) },
                onBack: { model.focusedResult = nil },
                onTapSnippet: { match in open(focused.id, match: match) }
            )
        } else {
            switch model.mode {
            case .content: contentResultsList
            case .path: pathResultsList
            }
        }
    }

    var contentResultsList: some View {
        ScrollViewReader { proxy in
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 0) {
                    ForEach(Array(model.contentResults.enumerated()), id: \.element.id) { index, result in
                        SearchResultRow(
                            result: result,
                            systemImage: model.icon(for: result.id, name: result.filename),
                            fetchSnippet: { match in model.snippet(id: result.id, match: match) },
                            onTap: {
                                model.selected = index
                                open(result.id, match: result.matches.first)
                            },
                            onShowMore: { model.focusedResult = result }
                        )
                        .background(selectionBackground(index))
                        .id(result.id)
                        Divider()
                    }
                }
            }
            .onChange(of: model.selected) { sel in
                scroll(proxy, to: model.contentResults[safe: sel]?.id)
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    var pathResultsList: some View {
        ScrollViewReader { proxy in
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 0) {
                    ForEach(Array(model.pathResults.enumerated()), id: \.element.id) { index, result in
                        PathSearcherRow(
                            result: result,
                            systemImage: model.icon(for: result.id, name: result.filename),
                            onTap: {
                                model.selected = index
                                open(result.id)
                            }
                        )
                        .background(selectionBackground(index))
                        .id(result.id)
                        Divider()
                    }
                }
            }
            .onChange(of: model.selected) { sel in
                scroll(proxy, to: model.pathResults[safe: sel]?.id)
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private func selectionBackground(_ index: Int) -> Color {
        model.selected == index ? Color.accentColor.opacity(0.15) : Color.clear
    }

    private func scroll(_ proxy: ScrollViewProxy, to id: UUID?) {
        guard let id else { return }
        withAnimation { proxy.scrollTo(id, anchor: .center) }
    }
}

private extension Array {
    subscript(safe index: Int) -> Element? {
        indices.contains(index) ? self[index] : nil
    }
}

struct SearchResultRow: View {
    let result: ContentSearcherResult
    let systemImage: String
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
        HStack(alignment: .top, spacing: 12) {
            Image(systemName: systemImage)
                .font(.title3)
                .foregroundColor(.accentColor)
                .frame(width: 22)
                .padding(.top, 1)

            VStack(alignment: .leading, spacing: 4) {
                highlightedPath(result.filename, byteBase: filenameByteBase)
                    .font(.body)
                    .fontWeight(.medium)
                highlightedPath(result.parentPath, byteBase: parentByteBase)
                    .font(.caption)
                    .foregroundColor(.secondary)

                if !result.matches.isEmpty {
                    VStack(alignment: .leading, spacing: 4) {
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
                        }
                    }
                    .padding(.horizontal, 10)
                    .padding(.vertical, 7)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .background(
                        RoundedRectangle(cornerRadius: 8)
                            .fill(Color.gray.opacity(0.15))
                    )
                    .padding(.top, 2)
                }
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(.horizontal, 12)
        .padding(.vertical, 8)
        .contentShape(Rectangle())
        .onTapGesture { onTap() }
    }

    private var parentByteBase: Int {
        result.parentPath == "/" ? 0 : 1
    }

    private var filenameByteBase: Int {
        if result.parentPath.isEmpty || result.parentPath == "/" {
            return 1
        }
        return result.parentPath.utf8.count + 2
    }

    private func highlightedPath(_ s: String, byteBase: Int) -> Text {
        var out = Text("")
        var byte = byteBase
        for scalar in s.unicodeScalars {
            let len = String(scalar).utf8.count
            let matched = result.pathMatches.contains { $0.rangeStart < byte + len && byte < $0.rangeEnd }
            let part = Text(String(scalar))
            out = out + (matched ? part.underline() : part)
            byte += len
        }
        return out
    }

    @ViewBuilder
    func snippetLine(for match: ContentSearcherMatch) -> some View {
        if let snippet = fetchSnippet(match) {
            (Text(snippet.prefix).foregroundColor(.secondary)
                + Text(snippet.matched).bold().foregroundColor(.primary)
                + Text(snippet.suffix).foregroundColor(.secondary))
                .font(.caption)
                .lineLimit(1)
                .truncationMode(.tail)
                .frame(maxWidth: .infinity, alignment: .leading)
        }
    }
}

struct PathSearcherRow: View {
    let result: PathSearcherResult
    let systemImage: String
    let onTap: () -> Void

    var body: some View {
        HStack(spacing: 12) {
            Image(systemName: systemImage)
                .font(.title3)
                .foregroundColor(.accentColor)
                .frame(width: 22)

            VStack(alignment: .leading, spacing: 4) {
                highlighted(result.filename, offset: filenameOffset)
                    .font(.body)
                if !result.parentPath.isEmpty, result.parentPath != "/" {
                    highlighted(result.parentPath, offset: parentOffset)
                        .font(.caption)
                        .foregroundColor(.secondary)
                }
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
    let systemImage: String
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

            Image(systemName: systemImage)
                .font(.title3)
                .foregroundColor(.accentColor)

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
            LazyVStack(alignment: .leading, spacing: 8) {
                ForEach(Array(result.matches.enumerated()), id: \.offset) { _, match in
                    Button(action: { onTapSnippet(match) }) {
                        snippetRow(for: match)
                    }
                    .buttonStyle(.plain)
                }
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 10)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    @ViewBuilder
    func snippetRow(for match: ContentSearcherMatch) -> some View {
        if let snippet = fetchSnippet(match) {
            (Text(snippet.prefix).foregroundColor(.secondary)
                + Text(snippet.matched).bold().foregroundColor(.primary)
                + Text(snippet.suffix).foregroundColor(.secondary))
                .font(.caption)
                .lineLimit(1)
                .truncationMode(.tail)
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(.horizontal, 10)
                .padding(.vertical, 8)
                .background(
                    RoundedRectangle(cornerRadius: 8)
                        .fill(Color.gray.opacity(0.15))
                )
                .contentShape(Rectangle())
        }
    }
}

class SearchViewModel: ObservableObject {
    @Published var input: String = ""
    @Published var mode: SearchMode = .platformDefault
    @Published var contentResults: [ContentSearcherResult] = []
    @Published var pathResults: [PathSearcherResult] = []
    @Published var focusedResult: ContentSearcherResult? = nil
    @Published var isQuerying: Bool = false
    /// Index of the keyboard-highlighted row within the current mode's result list.
    @Published var selected: Int = 0

    let filesModel: FilesViewModel

    private var contentSearcher: ContentSearching?
    private var pathSearcher: PathSearching?

    private let searchQueue = DispatchQueue(label: "lockbook.search")
    private var querySeq: UInt64 = 0

    init(filesModel: FilesViewModel) {
        self.filesModel = filesModel
    }

    /// Rebuild the indexes (picking up new/edited files) and re-run the current query.
    func refresh() {
        contentSearcher = AppState.lb.contentSearcher()
        pathSearcher = AppState.lb.pathSearcher()
        search()
    }

    func search() {
        querySeq &+= 1
        let seq = querySeq
        let mode = mode
        let input = input
        let contentSearcher = contentSearcher
        let pathSearcher = pathSearcher

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
                self.contentResults = content
                self.pathResults = path
                self.selected = 0
                self.isQuerying = false
            }
        }
    }

    /// Number of rows in the list currently shown (depends on the active mode).
    var resultCount: Int {
        mode == .content ? contentResults.count : pathResults.count
    }

    /// Move the keyboard highlight, clamped to the result list.
    func moveSelection(_ delta: Int) {
        guard resultCount > 0 else {
            selected = 0
            return
        }
        selected = min(max(0, selected + delta), resultCount - 1)
    }

    func snippet(id: UUID, match: ContentSearcherMatch) -> SearcherSnippet? {
        guard let contentSearcher else { return nil }
        return searchQueue.sync { contentSearcher.snippet(id: id, match: match, contextChars: 40) }
    }

    func icon(for id: UUID, name: String) -> String {
        if let file = filesModel.idsToFiles[id] {
            return FileIconHelper.fileToSystemImageName(file: file)
        }
        return FileIconHelper.docNameToSystemImageName(name: name)
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
