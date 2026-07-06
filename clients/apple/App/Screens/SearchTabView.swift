#if os(iOS)
    import SwiftUI
    import SwiftWorkspace

    struct SearchTabView: View {
        @Environment(HomeState.self) private var homeState
        @Environment(WorkspaceInputState.self) private var workspaceInput

        @Bindable var model: SearchModel
        @FocusState private var fieldFocused: Bool
        @State private var scrollPosition = ScrollPosition()

        var body: some View {
            VStack(spacing: 0) {
                SearchField(
                    placeholder: "Search",
                    text: $model.input,
                    focus: $fieldFocused,
                    onSubmit: { fieldFocused = false }
                )
                modePicker
                Divider()
                results
            }
            .navigationTitle("Search")
            .navigationBarTitleDisplayMode(.inline)
            .onKeyPress(.upArrow) { moveSelection(-1) }
            .onKeyPress(.downArrow) { moveSelection(1) }
            .onKeyPress(.return) { openSelectedViaKey() }
            .onAppear {
                model.refresh()
                fieldFocused = true
            }
            .onChange(of: model.input) { model.search() }
            .onChange(of: model.mode) { model.search() }
        }

        private func open(_ id: UUID, match: ContentSearcherMatch? = nil) {
            model.open(id: id, workspaceInput: workspaceInput, match: match)
            homeState.compactColumn = .detail
        }

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
            resultsList(model.contentResults) { index, result in
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
            }
        }

        var pathResultsList: some View {
            resultsList(model.pathResults) { index, result in
                PathSearcherRow(
                    result: result,
                    systemImage: model.icon(for: result.id, name: result.filename),
                    onTap: {
                        model.selected = index
                        open(result.id)
                    }
                )
            }
        }

        private func resultsList<R: Identifiable, Row: View>(
            _ results: [R], @ViewBuilder row: @escaping (Int, R) -> Row
        ) -> some View where R.ID == UUID {
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 0) {
                    ForEach(Array(results.enumerated()), id: \.element.id) { index, result in
                        VStack(spacing: 0) {
                            row(index, result)
                                .background(selectionBackground(index))

                            Divider()
                        }
                    }
                }
                .scrollTargetLayout()
            }
            .scrollPosition($scrollPosition)
            .onChange(of: model.selected) {
                scrollToSelection(in: results)
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        }

        private func selectionBackground(_ index: Int) -> Color {
            model.selected == index ? Color.accentColor.opacity(0.15) : Color.clear
        }

        private func scrollToSelection(in results: [some Identifiable<UUID>]) {
            guard let id = results.indices.contains(model.selected) ? results[model.selected].id : nil else {
                return
            }
            withAnimation { scrollPosition.scrollTo(id: id, anchor: .center) }
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
                out = Text("\(out)\(matched ? part.underline() : part)")
                byte += len
            }
            return out
        }

        @ViewBuilder
        func snippetLine(for match: ContentSearcherMatch) -> some View {
            if let snippet = fetchSnippet(match) {
                snippetText(snippet)
            }
        }
    }

    private func snippetText(_ snippet: SearcherSnippet) -> some View {
        Text("\(Text(snippet.prefix).foregroundColor(.secondary))\(Text(snippet.matched).bold().foregroundColor(.primary))\(Text(snippet.suffix).foregroundColor(.secondary))")
            .font(.caption)
            .lineLimit(1)
            .truncationMode(.tail)
            .frame(maxWidth: .infinity, alignment: .leading)
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
            result.parentPath == "/" ? 0 : 1
        }

        private var filenameOffset: Int {
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
                    out = Text("\(out)\(part.bold().foregroundColor(.primary))")
                } else {
                    out = Text("\(out)\(part.foregroundColor(.secondary))")
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
                snippetText(snippet)
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

    #Preview {
        NavigationStack {
            SearchTabView(model: .preview)
        }
        .environment(HomeState())
        .environment(WorkspaceInputState.preview)
    }
#endif
