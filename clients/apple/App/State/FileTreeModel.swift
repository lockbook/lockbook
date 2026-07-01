import Combine
import Foundation
import SwiftWorkspace

@Observable class FileTreeModel {
    var openFolders: Set<UUID> = []
    var openDoc: UUID? = nil

    @ObservationIgnored var suppressNextFolderSelection = false
    @ObservationIgnored private var cancellables: Set<AnyCancellable> = []

    private let filesModel: FilesModel

    init(filesModel: FilesModel, workspaceOutput: WorkspaceOutputState) {
        self.filesModel = filesModel

        workspaceOutput.$openDoc.sink { [weak self] openDoc in
            guard let self, let openDoc, let file = filesModel.idsToFiles[openDoc] else {
                return
            }

            self.openDoc = openDoc
            self.expandToFile(file)
        }
        .store(in: &cancellables)

        workspaceOutput.$selectedFolder.sink { [weak self] selectedFolder in
            guard let self else {
                return
            }

            if suppressNextFolderSelection {
                suppressNextFolderSelection = false
                return
            }

            guard let selectedFolder, let file = filesModel.idsToFiles[selectedFolder] else {
                return
            }

            expandToFile(file)
        }
        .store(in: &cancellables)
    }

    func toggleFolder(_ id: UUID) {
        if openFolders.remove(id) == nil {
            openFolders.insert(id)
        }
    }

    func expandToFile(_ file: File) {
        if file.isRoot {
            return
        }

        if let parent = filesModel.idsToFiles[file.parent] {
            expandToFile(parent)
        }

        openFolders.insert(file.id)
    }
}

extension FileTreeModel {
    static var preview: FileTreeModel {
        FileTreeModel(filesModel: .preview, workspaceOutput: .preview)
    }
}
