import SwiftWorkspace
import Foundation

enum FileOperationSheetInfo: Identifiable {
    case createFolder(parent: File)
    case rename(file: File)
    case share(file: File)
    
    var id: String {
        switch self {
        case .createFolder(let parent):
            return "createFolder-\(parent.id)"
        case .rename(let file):
            return "rename-\(file.id)"
//        case .select(let action):
//            return "select-\(action.id)"
        case .share(let file):
            return "share-\(file.id)"
        }
    }
}

enum SelectFolderAction: Identifiable {
    case move(files: [File])
    case externalImport(paths: [String])
    case acceptShare(name: String, id: UUID)
    
    var id: String {
        switch self {
        case .move(let ids):
            return "move-\(ids.map(\.name).joined(separator: ","))"
        case .externalImport(let paths):
            return "import-\(paths.joined(separator: ","))"
        case .acceptShare(let name, let id):
            return "acceptShare-\(name)-\(id.uuidString)"
        }
    }
}
