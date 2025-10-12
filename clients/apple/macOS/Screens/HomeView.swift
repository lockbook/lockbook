import SwiftUI
import SwiftWorkspace

struct HomeView: View {
    @StateObject var homeState: HomeState
    @EnvironmentObject var filesModel: FilesViewModel
    @EnvironmentObject var workspaceInput: WorkspaceInputState

    init(workspaceOutput: WorkspaceOutputState, filesModel: FilesViewModel) {
        self._homeState = StateObject(
            wrappedValue: HomeState(
                workspaceOutput: workspaceOutput,
                filesModel: filesModel
            )
        )
    }

    var body: some View {
        PathSearchContainerView(filesModel: filesModel, workspaceInput: workspaceInput) {
            NavigationSplitView(
                columnVisibility: homeState.splitViewVisibility,
                sidebar: {
                    SearchContainerView(filesModel: filesModel) {
                        SidebarView()
                    }
                },
                detail: {
                    NavigationStack {
                        DetailView()
                            .navigationDestination(
                                isPresented: $homeState.showPendingShares
                            ) {
                                PendingSharesView()
                            }
                            .modifier(OutOfSpaceAlert())
                    }
                }
            )
        }
        .confirmationDialog(
            "Are you sure? This action cannot be undone.",
            isPresented: Binding(
                get: { filesModel.deleteFileConfirmation != nil },
                set: { _ in filesModel.deleteFileConfirmation = nil }
            ),
            titleVisibility: .visible,
            actions: {
                if let files = filesModel.deleteFileConfirmation {
                    DeleteConfirmationButtons(files: files)
                }
            }
        )
        .navigationSplitViewStyle(.balanced)
        .environmentObject(homeState)
        .environmentObject(filesModel)
    }
}

struct SidebarView: View {
    @EnvironmentObject var homeState: HomeState
    @EnvironmentObject var filesModel: FilesViewModel

    @StateObject var settingsModel = SettingsViewModel()

    @Environment(\.colorScheme) var colorScheme

    var body: some View {
        if let error = filesModel.error {
            Text(error)
                .foregroundStyle(.red)
        } else if filesModel.loaded {
            Form {
                CollapsableSection(
                    id: "Suggested_Docs",
                    label: {
                        Label(
                            "Suggested Documents",
                            systemImage: "books.vertical.fill"
                        )
                        .bold()
                        .font(.callout)
                    },
                    content: {
                        SuggestedDocsView(filesModel: filesModel)
                    }
                )

                Section(
                    header:
                        Label("Files", systemImage: "folder")
                        .bold()
                        .padding(.horizontal)
                        .font(.callout)
                        .padding(.top, 8)
                ) {
                    FileTreeView()
                        .padding(.horizontal, 8)
                }

                Spacer()

                VStack(spacing: 0) {
                    UsageBar()
                        .environmentObject(settingsModel)
                        .padding(.horizontal, 12)

                    StatusBarView()
                }
            }
            .formStyle(.columns)
            .selectFolderSheets()
            .fileOpSheets(compactSheetHeight: .constant(0))
            .toolbar {
                ToolbarItem(placement: .primaryAction) {
                    Button(
                        action: {
                            homeState.showPendingShares = true
                        },
                        label: {
                            PendingSharesIcon(homeState: homeState)
                        }
                    )
                }
            }
        }
    }
}

struct DetailView: View {
    @Environment(\.isPreview) var isPreview

    @EnvironmentObject var homeState: HomeState
    @EnvironmentObject var workspaceInput: WorkspaceInputState
    @EnvironmentObject var workspaceOutput: WorkspaceOutputState

    var body: some View {
        if isPreview {
            Text("This is a preview.")
        } else {
            WorkspaceView(
                workspaceInput,
                workspaceOutput,
                AppState.lb.lbUnsafeRawPtr
            )
            .modifier(OnLbLinkViewModifier())
        }
    }
}

#Preview("Home View") {
    return HomeView(workspaceOutput: .preview, filesModel: .preview)
        .withCommonPreviewEnvironment()
}
