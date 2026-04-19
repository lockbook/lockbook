import SwiftUI
import SwiftWorkspace

class PathSearchViewModel: ObservableObject {
    @Published var input: String = ""
    @Published var isShown: Bool = false
    @Published var isSearchInProgress: Bool = false

    @Published var results: [PathSearcherResult] = []
    @Published var selected = 0

    let filesModel: FilesViewModel
    let workspaceInput: WorkspaceInputState

    private var searcher: PathSearching?

    init(filesModel: FilesViewModel, workspaceInput: WorkspaceInputState) {
        self.filesModel = filesModel
        self.workspaceInput = workspaceInput
    }

    func openSelected() {
        guard selected != -1 || (selected == -1 && !results.isEmpty) else {
            return
        }

        guard selected < results.count else {
            return
        }

        guard let file = filesModel.idsToFiles[results[selected].id] else {
            return
        }

        if file.type == .folder {
            workspaceInput.selectFolder(id: file.id)
        } else {
            workspaceInput.openFile(id: file.id)
        }

        endSearch()
    }

    func search() {
        selected = 0
        guard let searcher else { return }
        results = Array(searcher.query(input).prefix(20))
        selected = min(selected, results.count - 1)
    }

    func selectNextPath() {
        if results.count > 0 {
            selected = min(results.count - 1, selected + 1)
        }
    }

    func selectPreviousPath() {
        selected = max(0, selected - 1)
    }

    func toggleSearch() {
        if isShown {
            endSearch()
        } else {
            searcher = AppState.lb.pathSearcher()
            isShown = true
        }
    }

    func endSearch() {
        isShown = false
        searcher = nil
        results = []
        workspaceInput.focus.send(())
    }
}

extension PathSearchViewModel {
    static var preview: PathSearchViewModel {
        PathSearchViewModel(filesModel: .preview, workspaceInput: .preview)
    }
}
