import SwiftLockbookCore
import Combine
import Foundation

class CurrentDocument: ObservableObject {

    // TODO evaluate if this can be merged with DocumentLoader related state
    @Published var selectedDocument: File? {
        didSet {
            print("SET THIS \(selectedDocument?.name)")
            DI.documentLoader.meta = selectedDocument
            DI.documentLoader.loading = true
            selectedFolder = nil
            isPendingSharesOpen = false
        }
    }
    
    @Published var isPendingSharesOpen: Bool = false

    @Published var selectedFolder: File?
}
