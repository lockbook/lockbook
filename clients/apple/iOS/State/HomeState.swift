import SwiftUI
import SwiftWorkspace

class HomeState: ObservableObject {
    @Published var error: UIError? = nil
    @Published var fileActionCompleted: FileAction? = nil
    
    
    
}

public enum FileAction {
    case move
    case delete
    case createFolder
    case importFiles
    case acceptedShare
}

enum UIError {
    case lb(error: LbError)
    case custom(title: String, msg: String)
}
