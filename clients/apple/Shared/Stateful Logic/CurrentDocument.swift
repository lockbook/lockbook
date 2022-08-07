import SwiftLockbookCore
import Combine

class CurrentDocument: ObservableObject {

    // TODO evaluate if this can be merged with DocumentLoader related state
    @Published var selectedItem: File?

}