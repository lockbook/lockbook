import SwiftUI
import SwiftWorkspace

struct StatusBarView: View {
    @Environment(\.horizontalSizeClass) var horizontalSizeClass

    @EnvironmentObject var homeState: HomeState
    @EnvironmentObject var filesModel: FilesViewModel
    @EnvironmentObject var workspaceInput: WorkspaceInputState
    @EnvironmentObject var workspaceOutput: WorkspaceOutputState

    var body: some View {
        VStack {
            if filesModel.selectedFilesState.isSelectableState() {
                selectedFilesOptions
            } else {
                statusBar
            }
        }
        .frame(height: 35)
        .padding(8)
        .modifier(GlassEffectModifier())
        .padding(.bottom)
        .padding(.horizontal)
    }

    var selectedFilesOptions: some View {
        HStack(alignment: .center) {
            Spacer()

            Button(
                role: .destructive,
                action: {
                    filesModel.deleteFileConfirmation =
                        filesModel.getConsolidatedSelection()
                }
            ) {
                Image(systemName: "trash")
                    .imageScale(.large)
            }
            .disabled(filesModel.selectedFilesState.count == 0)

            Spacer()

            Button(
                action: {
                    homeState.selectSheetInfo = .move(
                        files: filesModel.getConsolidatedSelection()
                    )
                },
                label: {
                    Image(systemName: "folder")
                        .imageScale(.large)
                }
            )
            .disabled(filesModel.selectedFilesState.count == 0)

            Spacer()

            Button(
                action: {
                    self.exportFiles(
                        homeState: homeState,
                        files: filesModel.getConsolidatedSelection()
                    )
                    filesModel.selectedFilesState = .unselected
                },
                label: {
                    Image(systemName: "square.and.arrow.up")
                        .imageScale(.large)
                }
            )
            .disabled(filesModel.selectedFilesState.count == 0)

            Spacer()
        }
        .foregroundStyle(
            filesModel.selectedFilesState.count == 0 ? .gray : Color.accentColor
        )
        .padding(.horizontal)
    }

    var statusBar: some View {
        HStack {
            SyncButton()

            Spacer()

            fileActionButtons
        }
        .padding(.horizontal, 20)
    }

    var fileActionButtons: some View {
        HStack {
            if let root = filesModel.root {
                Button(action: {
                    self.docCreateAction {
                        workspaceInput.createDocAt(
                            parent: selectedFolderOrRoot(root).id,
                            drawing: false
                        )
                    }
                }) {
                    Image(systemName: "document.badge.plus.fill")
                        .font(.title2)
                        .foregroundColor(.accentColor)
                }
                .padding(.trailing, 5)

                Button(action: {
                    self.docCreateAction {
                        workspaceInput.createDocAt(
                            parent: selectedFolderOrRoot(root).id,
                            drawing: true
                        )
                    }
                }) {
                    Image(systemName: "pencil.tip.crop.circle.badge.plus.fill")
                        .font(.title2)
                        .foregroundColor(.accentColor)
                }
                .padding(.trailing, 2)

                Button(action: {
                    homeState.sheetInfo = .createFolder(
                        parent: selectedFolderOrRoot(root)
                    )
                }) {
                    Image(systemName: "folder.badge.plus.fill")
                        .font(.title2)
                        .foregroundColor(.accentColor)
                }
            } else {
                ProgressView()
            }
        }
    }

    func docCreateAction(f: () -> Void) {
        if homeState.isSidebarFloating {
            homeState.sidebarState = .closed
        }

        f()
    }

    func selectedFolderOrRoot(_ root: File) -> File {
        guard let selectedFolder = workspaceOutput.selectedFolder else {
            return root
        }

        return filesModel.idsToFiles[selectedFolder] ?? root
    }
}

struct GlassEffectModifier: ViewModifier {
    let radius: CGFloat = 20
    
    func body(content: Content) -> some View {
        if #available(iOS 26.0, *) {
            content
                .glassEffect(.regular)
        } else {
            content
        }
    }
}

#Preview {
    VStack {
        Spacer()

        StatusBarView()
            .withCommonPreviewEnvironment()
    }
}
