import SwiftUI
import SwiftWorkspace

enum SelectedFilesState {
    case unselected
    case selected(explicitly: Set<File>, implicitly: Set<File>)

    var count: Int {
        switch self {
        case .unselected:
            0
        case .selected(explicitly: let explicitly, implicitly: _):
            explicitly.count
        }
    }

    func isSelected(_ file: File) -> Bool {
        switch self {
        case .unselected:
            false
        case .selected(explicitly: _, implicitly: let implcitly):
            implcitly.contains(file)
        }
    }

    func isSelectableState() -> Bool {
        switch self {
        case .unselected:
            false
        case .selected(explicitly: _, implicitly: _):
            true
        }
    }
}
