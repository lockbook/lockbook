import Foundation
import SwiftWorkspace

enum FileOperationSheetInfo: Identifiable {
    case createFolder(parent: File)
    case rename(file: File)
    case share(file: File)
    case importPicker
    case camera

    var id: String {
        switch self {
        case let .createFolder(parent):
            "createFolder-\(parent.id)"
        case let .rename(file):
            "rename-\(file.id)"
//        case .select(let action):
//            return "select-\(action.id)"
        case let .share(file):
            "share-\(file.id)"
        case .importPicker:
            "importPicker"
        case .camera:
            "camera"
        }
    }
}

enum SelectFolderAction: Identifiable {
    case move(files: [File])
    case externalImport(urls: [URL])
    case acceptShare(name: String, id: UUID)

    var id: String {
        switch self {
        case let .move(ids):
            "move-\(ids.map(\.name).joined(separator: ","))"
        case let .externalImport(urls):
            "import-\(urls.map { $0.path(percentEncoded: false) }.joined(separator: ","))"
        case let .acceptShare(name, id):
            "acceptShare-\(name)-\(id.uuidString)"
        }
    }
}
