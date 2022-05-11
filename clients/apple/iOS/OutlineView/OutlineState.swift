import Combine
import SwiftLockbookCore

struct CreatingInfo {
    let parent: DecryptedFileMetadata
    let child_type: FileType
}

class OutlineState: ObservableObject {
    
    @Published var selectedItem: DecryptedFileMetadata?
    
    // These can't just be a part of OutlineContextMenu because the view goes away before
    // the sheet is presented
    @Published var creating: Bool = false
    @Published var creatingInfo: CreatingInfo? {
        didSet {
            if creatingInfo == nil {
                creating = false
            } else {
                creating = true
            }
        }
    }
    
    @Published var moving: Bool = false
    @Published var movingInfo: DecryptedFileMetadata? {
        didSet {
            if movingInfo == nil {
                moving = false
            } else {
                moving = true
            }
        }
    }
    
    @Published var renaming: Bool = false
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
