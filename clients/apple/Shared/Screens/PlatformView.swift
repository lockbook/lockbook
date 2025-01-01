import Foundation
import SwiftUI
import SwiftWorkspace
import AlertToast

struct PlatformView: View {
    @Environment(\.horizontalSizeClass) var horizontalSizeClass
    
    var body: some View {
        NavigationSplitView {
            FilesView()
        } detail: {
            WorkspaceView()
        }
        .navigationSplitViewStyle(.balanced)
    }
}
