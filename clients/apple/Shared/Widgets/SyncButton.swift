import SwiftUI

struct SyncButton: View {
    @State var syncButtonStatus: SyncButtonStatus = .canSync
    @Environment(\.isPreview) var isPreview
    
    var body: some View {
        Button(action: {
            AppState.workspaceState.requestSync()
        }, label: {
            if syncButtonStatus == .syncing {
                Label(title: { Text("Syncing...") }, icon: {
                    ProgressView()
                        .progressViewStyle(.automatic)
                        .modifier(SyncButtonProgressBarSize())
                })
            } else {
                Label("Sync", systemImage: getButtonIcon())
            }
        })
        .buttonStyle(.bordered)
        .tint(getButtonTintColor())
        .allowsHitTesting(!isButtonDisabled)
        .onReceive(AppState.lb.events.$status, perform: { status in
            guard !isPreview else { return }
            
            if status.offline {
                syncButtonStatus = .offline
            } else if status.syncing {
                syncButtonStatus = .syncing
            } else if status.outOfSpace {
                syncButtonStatus = .outOfSpace
            } else {
                syncButtonStatus = .canSync
            }
        })
    }
    
    var isButtonDisabled: Bool {
        get {
            return syncButtonStatus == .syncing || syncButtonStatus == .outOfSpace || syncButtonStatus == .updateRequired || syncButtonStatus == .offline
        }
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
