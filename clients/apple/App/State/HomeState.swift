import Observation
import SwiftUI

@Observable class HomeState {
    private static let compactColumnKey = "lastCompactColumnWasSidebar"

    var splitViewVisibility: NavigationSplitViewVisibility = .all

    var compactColumn: NavigationSplitViewColumn {
        didSet {
            UserDefaults.standard.set(compactColumn == .sidebar, forKey: Self.compactColumnKey)
        }
    }

    var openingLink = false

    var explicitSyncCount = 0

    init() {
        let wasSidebar = UserDefaults.standard.object(forKey: Self.compactColumnKey) as? Bool ?? true
        compactColumn = wasSidebar ? .sidebar : .detail
    }

    func explicitSyncRequested() {
        explicitSyncCount += 1
    }
}
