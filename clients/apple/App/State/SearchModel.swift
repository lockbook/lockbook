#if os(iOS)
    import Foundation
    import SwiftWorkspace

    enum SearchMode: String, CaseIterable, Identifiable {
        case path = "Filename"
        case content = "Content"

        var id: String { rawValue }
    }

    @Observable class SearchModel {
        var input: String = ""
        var mode: SearchMode = .path
        var contentResults: [ContentSearcherResult] = []
        var pathResults: [PathSearcherResult] = []
        var focusedResult: ContentSearcherResult? = nil
        var isQuerying: Bool = false
        /// Index of the keyboard-highlighted row within the current mode's result list.
        var selected: Int = 0

        @ObservationIgnored private var contentSearcher: ContentSearching?
        @ObservationIgnored private var pathSearcher: PathSearching?

        @ObservationIgnored private let searchQueue = DispatchQueue(label: "lockbook.search")
        @ObservationIgnored private var querySeq: UInt64 = 0

        private let filesModel: FilesModel

        init(filesModel: FilesModel) {
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

    extension SearchModel {
        static var preview: SearchModel {
            SearchModel(filesModel: .preview)
        }
    }
#endif
