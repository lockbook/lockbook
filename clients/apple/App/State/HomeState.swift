import Observation
import SwiftUI

@Observable class HomeState {
    #if os(iOS)
        var sidebarState: SidebarState = .closed
    #else
        var sidebarState: SidebarState = .open
    #endif

    var compactColumn: NavigationSplitViewColumn = .detail

    var splitViewVisibility: Binding<NavigationSplitViewVisibility> {
        Binding(
            get: {
                switch self.sidebarState {
                case .open:
                    .all
                case .closed:
                    .detailOnly
                }
            },
            set: { newVisibility in
                switch newVisibility {
                case .all:
                    self.sidebarState = .open
                case .detailOnly:
                    self.sidebarState = .closed
                default:
                    break
                }
            }
        )
    }
}

enum SidebarState {
    case closed
    case open
}
