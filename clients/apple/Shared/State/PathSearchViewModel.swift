import SwiftUI
import SwiftWorkspace

class PathSearchViewModel: ObservableObject {
    @Published var input: String = ""
    @Published var isShown: Bool = false
    @Published var isSearchInProgress: Bool = false

    @Published var results: [PathSearchResult] = []
    @Published var selected = 0

    let filesModel: FilesViewModel
    let workspaceInput: WorkspaceInputState

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

        DispatchQueue.global(qos: .userInitiated).async {
            let res = AppState.lb.search(input: self.input, searchPaths: true, searchDocs: false)

            DispatchQueue.main.async {
                switch res {
                case let .success(results):
                    self.results = results.map {
                        switch $0 {
                        case .document:
                            nil
                        case let .path(pathSearchResult):
                            pathSearchResult
                        }
                    }.compactMap { $0 }.prefix(20).sorted()

                    self.selected = min(self.selected, results.count - 1)
                case let .failure(err):
                    print("got error: \(err.msg)")
                }
            }
        }
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
            isShown = true
        }
    }

    func endSearch() {
        isShown = false
        workspaceInput.focus.send(())
    }
}

extension PathSearchViewModel {
    static var preview: PathSearchViewModel {
        PathSearchViewModel(filesModel: .preview, workspaceInput: .preview)
    }
}
