import Foundation
import SwiftLockbookCore

class UnexpectedErrorService: ObservableObject {
    @Published var globalError: AnyFfiError?
    
    func handleError(_ error: AnyFfiError) {
        DispatchQueue.main.async {
            self.globalError = error
        }
    }
}
