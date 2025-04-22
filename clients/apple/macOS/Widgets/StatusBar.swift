import SwiftUI
import SwiftWorkspace

struct StatusBar: View {
    @EnvironmentObject var workspaceState: WorkspaceState
    
    var body: some View {
        HStack {
            Text(workspaceState.statusMsg.isEmpty ? "..." : workspaceState.statusMsg)
                .lineLimit(1)
            
            Spacer()
            
            if !workspaceState.syncing {
                Button(action: {
                    workspaceState.requestSync()
                }) {
                    Text("Sync now")
                        .font(.callout)
                        .lineLimit(1)
                }
                .buttonStyle(.borderless)
            }
        }
        .padding(8)
        .cardBackground(background: Color.accentColor.opacity(0.2))
        .padding(8)
    }
}

#Preview {
    let workspaceState = WorkspaceState()
    workspaceState.statusMsg = "Just synced!"
    
    return StatusBar()
        .environmentObject(workspaceState)
}
