import SwiftUI
import SwiftWorkspace

let documentWindowId = "document"

struct DocumentWindowView: View {
    let fileId: UUID

    @State private var workspaceInput = WorkspaceInputState(coreHandle: AppState.lb.lbUnsafeRawPtr)
    @State private var workspaceOutput = WorkspaceOutputState()

    var body: some View {
        Group {
            if AppState.shared.isLoggedIn {
                WorkspaceView()
                    .frame(minWidth: 320, minHeight: 240)
                    .ignoresSafeArea(.keyboard)
                    .environment(workspaceInput)
                    .environment(workspaceOutput)
                    .onAppear {
                        workspaceInput.openFile(id: fileId, newTab: true)
                    }
            }
        }
        .navigationTitle(fileName)
    }

    private var fileName: String {
        (try? AppState.lb.getFile(id: fileId).get())?.name ?? "Lockbook"
    }
}
