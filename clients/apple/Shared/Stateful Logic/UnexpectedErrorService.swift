import Foundation
import SwiftLockbookCore

class UnexpectedErrorService: ObservableObject {
    @Published var globalError: AnyFfiError?
    
    func handleError(_ error: AnyFfiError) {
        DispatchQueue.main.async {
            self.globalError = error
        }
    }
    
    func errorWithTitle(_ title: String, _ message: String) {
        DispatchQueue.main.async {
            self.globalError = ErrorWithTitle(title: title, message: message)
        }
    }
}
