import Combine
import SwiftLockbookCore

struct CreatingInfo {
    let parent: ClientFileMetadata
    let child_type: FileType
}

class OutlineState: ObservableObject {
    
    @Published var selectedItem: ClientFileMetadata?
    @Published var dragging: ClientFileMetadata?
    @Published var renaming: ClientFileMetadata?
    @Published var creating: CreatingInfo?
}
