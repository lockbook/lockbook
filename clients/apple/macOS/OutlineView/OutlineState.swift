import Combine
import SwiftLockbookCore

class OutlineState: ObservableObject {
    
    @Published var selectedItem: ClientFileMetadata?
    @Published var dragging: ClientFileMetadata?
    @Published var renaming: ClientFileMetadata?

}
