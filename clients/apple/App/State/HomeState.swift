import Observation
import SwiftUI

@Observable class HomeState {
    #if os(iOS)
        var splitViewVisibility: NavigationSplitViewVisibility = .detailOnly
    #else
        var splitViewVisibility: NavigationSplitViewVisibility = .all
    #endif

    var compactColumn: NavigationSplitViewColumn = .detail
}
