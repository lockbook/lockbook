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
            creating = creatingInfo != nil
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
            moving = movingInfo != nil
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
            renaming = renamingInfo != nil
        }
    }
    
    @Published var acceptingShare: Bool = false {
        didSet {
            if !acceptingShare && acceptingShareInfo != nil {
                acceptingShareInfo = nil
            }
        }
    }
    @Published var acceptingShareInfo: File? {
        didSet {
            acceptingShare = acceptingShareInfo != nil
        }
    }
    
    @Published var sharingFile: Bool = false {
        didSet {
            if !sharingFile && sharingFileInfo != nil {
                sharingFileInfo = nil
            }
        }
    }
    @Published var sharingFileInfo: File? {
        didSet {
            sharingFile = sharingFileInfo != nil
        }
    }
}
