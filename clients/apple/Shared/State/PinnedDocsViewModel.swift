import Combine
import SwiftUI
import SwiftWorkspace

class PinnedDocsViewModel: ObservableObject {
    @Published var pinnedDocs: [PinnedDocInfo]? = nil

    var filesModel: FilesViewModel

    var cancellables: Set<AnyCancellable> = []

    init(filesModel: FilesViewModel) {
        self.filesModel = filesModel

        Publishers.CombineLatest(filesModel.$files, filesModel.$pinnedIds)
            .sink { [weak self] _, pinnedIds in
                self?.loadPinnedDocs(pinnedIds: pinnedIds)
            }
            .store(in: &cancellables)
    }

    func loadPinnedDocs(pinnedIds: [UUID]) {
        pinnedDocs = pinnedIds.compactMap { id in
            guard let file = filesModel.idsToFiles[id],
                  let parent = filesModel.idsToFiles[file.parent]
            else {
                return .none
            }

            return .some(PinnedDocInfo(
                name: file.name,
                id: file.id,
                parentName: parent.name,
                lastModified: AppState.lb.getTimestampHumanString(timestamp: Int64(file.lastModified))
            ))
        }
    }

    func unpinDoc(id: UUID) {
        filesModel.togglePin(id: id)
    }
}

struct PinnedDocInfo: Identifiable {
    let name: String
    let id: UUID
    let parentName: String
    let lastModified: String
}

extension PinnedDocsViewModel {
    static var preview: PinnedDocsViewModel {
        PinnedDocsViewModel(filesModel: .preview)
    }
}
