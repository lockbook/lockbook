import Combine
import SwiftLockbookCore

struct CreatingInfo {
    let parent: DecryptedFileMetadata
    let child_type: FileType
}

class OutlineState: ObservableObject {
    
    @Published var selectedItem: DecryptedFileMetadata?
    @Published var dragging: DecryptedFileMetadata?
    @Published var renaming: DecryptedFileMetadata?
    @Published var creating: CreatingInfo?
}
