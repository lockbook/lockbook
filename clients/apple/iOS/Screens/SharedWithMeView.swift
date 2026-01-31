import SwiftUI
import SwiftWorkspace

struct SharedWithMeView: View {
    @EnvironmentObject var homeState: HomeState
    @EnvironmentObject var filesModel: FilesViewModel

    @StateObject var fileTreeModel: FileTreeViewModel

    init(
        filesModel: FilesViewModel,
        workspaceInput: WorkspaceInputState,
        workspaceOutput: WorkspaceOutputState
    ) {
        self._fileTreeModel = StateObject(
            wrappedValue: FileTreeViewModel(
                filesModel: filesModel,
                workspaceInput: workspaceInput,
                workspaceOutput: workspaceOutput
            )
        )
    }

    var body: some View {
        Group {
            if let pendingShares = filesModel.pendingSharesByUsername {
                if pendingShares.isEmpty {
                    noShares
                } else {
                    sharedByUsers(pendingShares: pendingShares)
                }
            } else {
                ProgressView()
            }
        }
        .navigationTitle("Shared with me")
        .toolbarTitleDisplayMode(.large)
    }
    
    @ViewBuilder
    func sharedByUsers(pendingShares: [String: [File]]) -> some View {
        ScrollViewReader { scrollHelper in
            ScrollView {
                VStack {
                    ForEach(
                        pendingShares.sorted(by: { $0.key < $1.key }),
                        id: \.key
                    ) {
                        username,
                        shares in
                        SharedByUserSection(
                            username: username,
                            shares: shares,
                        )
                        .environmentObject(
                            fileTreeModel
                        )
                    }
                }
                .formStyle(.columns)
            }
            .onChange(of: fileTreeModel.openDoc) { newValue in
                scrollHelper.scrollTo(newValue, anchor: .center)
            }
        }
    }
    
    var noShares: some View {
        VStack {
            Spacer()
            
            VStack(spacing: 6) {
                Text("Nothing shared yet")
                    .font(.title3)
                    .fontWeight(.semibold)

                Text("Files shared with you will appear here.")
                    .font(.body)
                    .foregroundStyle(.secondary)
            }
            .multilineTextAlignment(.center)
            
            Spacer()
        }
    }
}

#Preview("Pending Shares") {
    NavigationStack {
        SharedWithMeView(
            filesModel: .preview,
            workspaceInput: .preview,
            workspaceOutput: .preview
        )
        .withMacPreviewSize()
        .withCommonPreviewEnvironment()
    }
}
