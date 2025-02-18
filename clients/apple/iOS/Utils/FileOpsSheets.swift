import SwiftUI
import SwiftWorkspace

extension View {
    func fileOpSheets(
        constrainedSheetHeight: Binding<CGFloat>
    ) -> some View {
        modifier(FileOpSheets(constrainedSheetHeight: constrainedSheetHeight))
    }
}


struct FileOpSheets: ViewModifier {
    @Environment(\.isConstrainedLayout) var isConstrainedLayout
    @EnvironmentObject var homeState: HomeState
    @EnvironmentObject var workspaceState: WorkspaceState
    @EnvironmentObject var filesModel: FilesViewModel
    
    @Binding var constrainedSheetHeight: CGFloat
    
    func body(content: Content) -> some View {
        if isConstrainedLayout {
            content
                .sheet(item: $homeState.sheetInfo) { info in
                    switch info {
                    case .createFolder(parent: let parent):
                        CreateFolderSheet(homeState: homeState, workspaceState: workspaceState, parentId: parent.id)
                            .autoSizeSheet(sheetHeight: $constrainedSheetHeight)
                    case .rename(file: let file):
                        RenameFileSheet(homeState: homeState, workspaceState: workspaceState, id: file.id, name: file.name)
                            .autoSizeSheet(sheetHeight: $constrainedSheetHeight)
                    case .share(file: let file):
                        ShareFileSheet(workspaceState: workspaceState, id: file.id, name: file.name, shares: file.shares)
                            .autoSizeSheet(sheetHeight: $constrainedSheetHeight)
                    }
                }
                .sheet(item: $homeState.selectSheetInfo) { action in
                    SelectFolderSheet(homeState: homeState, filesModel: filesModel, action: action)
                }
        } else {
            content
                .formSheet(item: $homeState.sheetInfo) { info in
                    switch info {
                    case .createFolder(parent: let parent):
                        CreateFolderSheet(homeState: homeState, workspaceState: workspaceState, parentId: parent.id)
                            .frame(width: CreateFolderSheet.FORM_WIDTH, height: CreateFolderSheet.FORM_HEIGHT)
                    case .rename(file: let file):
                        RenameFileSheet(homeState: homeState, workspaceState: workspaceState, id: file.id, name: file.name)
                            .frame(width: RenameFileSheet.FORM_WIDTH, height: RenameFileSheet.FORM_HEIGHT)
                    case .share(file: let file):
                        ShareFileSheet(workspaceState: workspaceState, id: file.id, name: file.name, shares: file.shares)
                            .frame(width: ShareFileSheet.FORM_WIDTH, height: ShareFileSheet.FORM_HEIGHT)
                    }
                }
                .sheet(item: $homeState.selectSheetInfo) { action in
                    SelectFolderSheet(homeState: homeState, filesModel: filesModel, action: action)
                }
        }
    }
}
