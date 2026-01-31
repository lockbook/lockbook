import SwiftUI
import SwiftWorkspace

struct FileRowContextMenu: View {
    let file: File

    @EnvironmentObject var filesModel: FilesViewModel
    @EnvironmentObject var homeState: HomeState
    @EnvironmentObject var workspaceInput: WorkspaceInputState

    var isSharedFile: Bool {
        filesModel.pendingSharesAndChildren.contains(file.id)
    }

    var body: some View {
        if isSharedFile {
            sharedFileActions
        } else {
            ownedFileActions
        }
    }

    var sharedFileActions: some View {
        Group {
            Button(action: {
                exportFiles(homeState: homeState, files: [file])
            }) {
                Label(
                    "Share externally to...",
                    systemImage: "square.and.arrow.up.fill"
                )
            }

            if file.type == .document {
                Button(action: {
                    ClipboardHelper.copyFileLink(file.id)
                }) {
                    Label("Copy file link", systemImage: "link")
                }
            }
        }
    }

    var ownedFileActions: some View {
        Group {
            if file.isFolder {
                Button(action: {
                    workspaceInput.createDocAt(parent: file.id, drawing: false)
                }) {
                    Label("Create a document", systemImage: "doc.fill")
                }
                Button(action: {
                    workspaceInput.createDocAt(parent: file.id, drawing: true)
                }) {
                    Label(
                        "Create a drawing",
                        systemImage: "pencil.tip.crop.circle.badge.plus"
                    )
                }
                Button(action: {
                    homeState.sheetInfo = .createFolder(parent: file)
                }) {
                    Label("Create a folder", systemImage: "folder.fill")
                }
            }

            if !file.isRoot {
                Button(action: {
                    homeState.sheetInfo = .rename(file: file)
                }) {
                    Label("Rename", systemImage: "pencil.circle.fill")
                }

                Button(action: {
                    homeState.selectSheetInfo = .move(files: [file])
                }) {
                    Label(
                        "Move",
                        systemImage:
                            "arrow.up.and.down.and.arrow.left.and.right"
                    )
                }

                Divider()

                Button(action: {
                    homeState.sheetInfo = .share(file: file)
                }) {
                    Label("Share", systemImage: "person.wave.2.fill")
                }

                Button(action: {
                    exportFiles(homeState: homeState, files: [file])
                }) {
                    Label(
                        "Share externally to...",
                        systemImage: "square.and.arrow.up.fill"
                    )
                }

                if file.type == .document {
                    Button(action: {
                        ClipboardHelper.copyFileLink(file.id)
                    }) {
                        Label("Copy file link", systemImage: "link")
                    }
                }

                Divider()

                Button(
                    role: .destructive,
                    action: {
                        filesModel.deleteFileConfirmation = [file]
                    }
                ) {
                    Label("Delete", systemImage: "trash.fill")
                }
            }
        }
    }
}
