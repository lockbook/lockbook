import SwiftUI
import SwiftWorkspace

enum SelectedFilesState {
    case unselected
    case selected(explicitly: Set<File>, implicitly: Set<File>)
    
    var count: Int {
        get {
            switch self {
                
            case .unselected:
                return 0
            case .selected(explicitly: let explicitly, implicitly: _):
                return explicitly.count
            }
        }
    }
    
    func isSelected(_ file: File) -> Bool {
        switch self {
        case .unselected:
            return false
        case .selected(explicitly: _, implicitly: let implcitly):
            return implcitly.contains(file)
        }
    }
    
    func isSelectableState() -> Bool {
        switch self {
        case .unselected:
            return false
        case .selected(explicitly: _, implicitly: _):
            return true
        }
    }
}
