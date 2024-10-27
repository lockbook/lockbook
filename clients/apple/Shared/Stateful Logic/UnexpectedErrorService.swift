import Foundation
import SwiftWorkspace

class UnexpectedErrorService: ObservableObject {
    @Published var globalError: LbError?
    
    func handleError(_ error: LbError) {
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
