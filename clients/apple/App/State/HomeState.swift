import Observation
import SwiftUI

@Observable class HomeState {
    var splitViewVisibility: NavigationSplitViewVisibility = .all

    var compactColumn: NavigationSplitViewColumn = .detail

    var openingLink = false

    var explicitSyncCount = 0

    func explicitSyncRequested() {
        explicitSyncCount += 1
    }
}
