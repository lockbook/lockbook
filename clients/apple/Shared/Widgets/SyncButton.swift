import SwiftUI
import AlertToast

struct SyncButton: View {
    @EnvironmentObject var homeState: HomeState
    @State var syncButtonStatus: SyncButtonStatus = .canSync
    @State private var showClickToast = false

    @Environment(\.isPreview) var isPreview
    
    var body: some View {
        Button(action: {
            if syncButtonStatus == .updateRequired {
                AppState.shared.error = .custom(title: "Your Lockbook is out of date", msg: "Update to the latest version to sync")
            } else if syncButtonStatus == .outOfSpace {
                homeState.showOutOfSpaceAlert = true
            }
            
            AppState.workspaceState.requestSync()
        }, label: {
            if syncButtonStatus == .syncing {
                Label(title: { Text("Sync") }, icon: {
                    ProgressView()
                        .progressViewStyle(.circular)
                        .padding(.trailing, 1)
                        .modifier(SyncButtonProgressBarSize())
                }).padding(.vertical, 5)
            } else {
                Label("Sync", systemImage: getButtonIcon()).padding(.vertical, 5)
            }
        })
        .buttonStyle(.bordered)
        .tint(getButtonTintColor())
        .onReceive(AppState.lb.events.$status, perform: { status in
            guard !isPreview else { return }
            
            if status.offline {
                syncButtonStatus = .offline
            } else if status.outOfSpace {
                syncButtonStatus = .outOfSpace
            } else if status.syncing {
                syncButtonStatus = .syncing
            } else {
                syncButtonStatus = .canSync
            }
        })
    }
        
    func getButtonTintColor() -> Color? {
        return syncButtonStatus == .updateRequired ? .red : nil
    }
    
    func getButtonIcon() -> String {
        switch syncButtonStatus {
        case .offline:
            return "wifi.slash"
        case .canSync:
            return "arrow.triangle.2.circlepath"
        case .outOfSpace:
            return "gauge.high"
        case .updateRequired:
            return "exclamationmark.triangle.fill"
        case .syncing:
            // Should never be reached
            return "arrow.triangle.2.circlepath"
        }
    }
}

struct SyncButtonProgressBarSize: ViewModifier {
    func body(content: Content) -> some View {
        #if os(macOS)
        content.controlSize(.small)
        #else
        content
        #endif
    }
}

enum SyncButtonStatus {
    case offline
    case canSync
    case outOfSpace
    case updateRequired
    case syncing
}

#Preview("Can Sync") {
    SyncButton(syncButtonStatus: .canSync)
}

#Preview("Syncing") {
    SyncButton(syncButtonStatus: .syncing)
}

#Preview("Offline") {
    SyncButton(syncButtonStatus: .offline)
}

#Preview("Update required") {
    SyncButton(syncButtonStatus: .updateRequired)
}
