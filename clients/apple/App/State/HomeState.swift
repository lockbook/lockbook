import Observation
import SwiftUI

@Observable class HomeState {
    var splitViewVisibility: NavigationSplitViewVisibility = .all

    var compactColumn: NavigationSplitViewColumn = .detail

    var openingLink = false
}
