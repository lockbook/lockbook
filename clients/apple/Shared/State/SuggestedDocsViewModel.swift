import Combine
import SwiftUI
import SwiftWorkspace

class SuggestedDocsViewModel: ObservableObject {
    @Published var suggestedDocs: [SuggestedDocInfo]? = nil

    var filesModel: FilesViewModel

    var cancellables: Set<AnyCancellable> = []

    init(filesModel: FilesViewModel) {
        self.filesModel = filesModel

        filesModel.$files.sink { [weak self] _ in
            self?.loadSuggestedDocs()
        }
        .store(in: &cancellables)
    }

    func loadSuggestedDocs() {
        DispatchQueue.global(qos: .userInitiated).async {
            let res = AppState.lb.suggestedDocs()

            DispatchQueue.main.async {
                switch res {
                case let .success(ids):
                    let files = ids.compactMap { self.filesModel.idsToFiles[$0] }

                    self.suggestedDocs = files.prefix(20).compactMap { file in
                        guard let parent = self.filesModel.idsToFiles[file.parent] else {
                            return .none
                        }

                        return .some(SuggestedDocInfo(
                            name: file.name,
                            id: file.id,
                            parentName: parent.name,
                            lastModified: AppState.lb.getTimestampHumanString(timestamp: Int64(file.lastModified))
                        ))
                    }
                case .failure:
                    print("ignored for now")
                }
            }
        }
    }

    func clearSuggestedDoc(id: UUID) {
        DispatchQueue.global(qos: .userInitiated).async {
            let res = AppState.lb.clearSuggestedId(id: id)

            switch res {
            case .success:
                self.loadSuggestedDocs()
            case .failure:
                print("FAILURE WHILE CLEARING SUGGESTED DOCS IGNORED")
            }
        }
    }

    func clearSuggestedDocs() {
        DispatchQueue.global(qos: .userInitiated).async {
            let res = AppState.lb.clearSuggestedDocs()

            switch res {
            case .success:
                self.loadSuggestedDocs()
            case .failure:
                print("FAILURE WHILE CLEARING SUGGESTED DOCS IGNORED")
            }
        }
    }
}

struct SuggestedDocInfo: Identifiable {
    let name: String
    let id: UUID
    let parentName: String
    let lastModified: String
}

extension SuggestedDocsViewModel {
    static var preview: SuggestedDocsViewModel {
        SuggestedDocsViewModel(filesModel: .preview)
    }
}
