import Combine
import SwiftLockbookCore

enum ClientFileTypes {
    case Document
    case Folder
    case Drawing
}

struct CreatingInfo {
    let parent: DecryptedFileMetadata
    let child_type: FileType // TODO maybe pop out?
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
    @Published var created: DecryptedFileMetadata?
    
    @Published var moving: Bool = false {
        didSet {
            if !moving && movingInfo != nil {
                movingInfo = nil
            }
        }
    }
    @Published var movingInfo: DecryptedFileMetadata? {
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
    @Published var renamingInfo: DecryptedFileMetadata? {
        didSet {
            if renamingInfo == nil {
                renaming = false
            } else {
                renaming = true
            }
        }
    }
}
