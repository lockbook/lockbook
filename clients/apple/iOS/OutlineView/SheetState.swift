import Combine
import SwiftLockbookCore

enum ClientFileTypes {
    case Document
    case Folder
    case Drawing
}

struct CreatingInfo {
    let parent: File
    let child_type: FileType // TODO maybe pop out?
}

extension CreatingInfo {
    func toClientFileTypes() -> ClientFileTypes {
        switch child_type {
            case .Document:
                return ClientFileTypes.Document
            case .Folder:
                return ClientFileTypes.Folder
        }
    }
}

class SheetState: ObservableObject {
    // These can't just be a part of OutlineContextMenu because the view goes away before
    // the sheet is presented
    @Published var creating: Bool = false {
        didSet {
            if !creating && creatingInfo != nil {
                creatingInfo = nil
            }
        }
    }
    @Published var creatingInfo: CreatingInfo? {
        didSet {
            if creatingInfo == nil {
                creating = false
            } else {
                creating = true
            }
        }
    }
    @Published var created: File?
    
    @Published var moving: Bool = false {
        didSet {
            if !moving && movingInfo != nil {
                movingInfo = nil
            }
        }
    }
    @Published var movingInfo: File? {
        didSet {
            if movingInfo == nil {
                moving = false
            } else {
                moving = true
            }
        }
    }
    
    @Published var renaming: Bool = false {
        didSet {
            if !renaming && renamingInfo != nil {
                renamingInfo = nil
            }
        }
    }
    @Published var renamingInfo: File? {
        didSet {
            if renamingInfo == nil {
                renaming = false
            } else {
                renaming = true
            }
        }
    }
}
