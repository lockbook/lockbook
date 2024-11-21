import Foundation
import SwiftWorkspace

class ErrorService: ObservableObject {
    @Published var error: LbError?
    @Published var errorWithTitle: (String, String)? = nil
    
    func showError(_ error: LbError) {
        DispatchQueue.main.async {
            self.error = error
        }
    }
    
    func showErrorWithTitle(_ title: String, _ message: String) {
        DispatchQueue.main.async {
            self.errorWithTitle = (title, message)
        }
    }
}
