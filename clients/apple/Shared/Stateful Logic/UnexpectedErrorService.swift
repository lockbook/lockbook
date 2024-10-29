import Foundation
import SwiftWorkspace

class UnexpectedErrorService: ObservableObject {
    @Published var globalError: LbError?
    @Published var errorWithTitle: (String, String)? = nil
    
    func handleError(_ error: LbError) {
        DispatchQueue.main.async {
            self.globalError = error
        }
    }
    
    func errorWithTitle(_ title: String, _ message: String) {
        DispatchQueue.main.async {
            self.errorWithTitle = (title, message)
        }
    }
}
