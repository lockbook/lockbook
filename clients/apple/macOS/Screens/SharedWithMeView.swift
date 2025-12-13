import SwiftUI

import Combine
import SwiftWorkspace
import AppKit


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
            if let pendingShares = filesModel.pendingShares {
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
                                    fileRow: { file in
                                        PendingShareRowView(
                                            file: file,
                                        )
                                        .environmentObject(
                                            fileTreeModel
                                        )
                                    }
                                )
                            }
                        }
                        .formStyle(.columns)
                    }
                    .onChange(of: fileTreeModel.openDoc) { newValue in
                        scrollHelper.scrollTo(newValue, anchor: .center)
                    }
                }
            } else {
                ProgressView()
            }
        }
        .navigationTitle("Shared with me")
    }
}

