import SwiftUI
import SwiftWorkspace

class ErrorState: ObservableObject {
    @Published var error: UIError? = nil
}

enum UIError {
    case lb(error: LbError)
    case custom(title: String, msg: String)
}
