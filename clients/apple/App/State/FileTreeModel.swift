import Foundation
import SwiftUI
import SwiftWorkspace

@Observable class FileTreeModel {
    var openFolders: Set<UUID> = []
    var dropTarget: File? = nil

    let selection = SelectionModel()

    private let filesModel: FilesModel

    init(filesModel: FilesModel) {
        self.filesModel = filesModel
    }

    var visibleRows: [FileTreeRow] {
        guard let root = filesModel.root else {
            return []
        }

        var rows: [FileTreeRow] = []

        func walk(_ parent: UUID, level: CGFloat) {
            for child in filesModel.childrenByParent[parent] ?? [] {
                rows.append(FileTreeRow(file: child, level: level))

                if openFolders.contains(child.id) {
                    walk(child.id, level: level + 1)
                }
            }
        }

        walk(root.id, level: 0)
        return rows
    }

    func toggleFolder(_ id: UUID) {
        if openFolders.remove(id) == nil {
            openFolders.insert(id)
        }
    }

    func moveAndReveal(_ files: [File], into folder: File) -> Bool {
        guard filesModel.moveFiles(files, into: folder) else {
            return false
        }

        withAnimation {
            _ = openFolders.insert(folder.id)
        }

        return true
    }

    func open(_ file: File, workspaceInput: WorkspaceInputState, homeState: HomeState) {
        if file.isFolder {
            workspaceInput.selectFolder(id: file.id)

            withAnimation {
                toggleFolder(file.id)
            }
        } else {
            workspaceInput.openFile(id: file.id)
            homeState.compactColumn = .detail
        }
    }

    func docOpened(_ id: UUID) {
        guard let file = filesModel.idsToFiles[id] else {
            return
        }

        expandToFile(file)
    }

    func folderSelected(_ id: UUID) {
        guard let file = filesModel.idsToFiles[id],
              let parent = filesModel.idsToFiles[file.parent]
        else {
            return
        }

        expandToFile(parent)
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

struct FileTreeRow: Identifiable {
    let file: File
    let level: CGFloat

    var id: UUID {
        file.id
    }
}

extension FileTreeModel {
    static var preview: FileTreeModel {
        FileTreeModel(filesModel: .preview)
    }
}
