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
        
    @Binding var constrainedSheetHeight: CGFloat
    
    func body(content: Content) -> some View {
        // A little bit odd but not too bad
        #if os(iOS)
        if isConstrainedLayout {
            content
                .sheet(item: $homeState.sheetInfo) { info in
                    switch info {
                    case .createFolder(parent: let parent):
                        CreateFolderSheet(homeState: homeState, parentId: parent.id)
                            .autoSizeSheet(sheetHeight: $constrainedSheetHeight)
                    case .rename(file: let file):
                        RenameFileSheet(homeState: homeState, id: file.id, name: file.name)
                            .autoSizeSheet(sheetHeight: $constrainedSheetHeight)
                    case .share(file: let file):
                        ShareFileSheet(id: file.id, name: file.name, shares: file.shares)
                            .autoSizeSheet(sheetHeight: $constrainedSheetHeight)
                    case .importPicker:
                        ImportFilePicker()
                    }
                }
        } else {
            content
                .formSheet(item: $homeState.sheetInfo) { info in
                    switch info {
                    case .createFolder(parent: let parent):
                        CreateFolderSheet(homeState: homeState, parentId: parent.id)
                            .frame(width: CreateFolderSheet.FORM_WIDTH, height: CreateFolderSheet.FORM_HEIGHT)
                    case .rename(file: let file):
                        RenameFileSheet(homeState: homeState, id: file.id, name: file.name)
                            .frame(width: RenameFileSheet.FORM_WIDTH, height: RenameFileSheet.FORM_HEIGHT)
                    case .share(file: let file):
                        ShareFileSheet(id: file.id, name: file.name, shares: file.shares)
                            .frame(width: ShareFileSheet.FORM_WIDTH, height: ShareFileSheet.FORM_HEIGHT)
                    case .importPicker:
                        ImportFilePicker()
                    }
                }
        }
        #else
        content
            .sheet(item: $homeState.sheetInfo) { info in
                switch info {
                case .createFolder(parent: let parent):
                    CreateFolderSheet(homeState: homeState, parentId: parent.id)
                        .frame(width: CreateFolderSheet.FORM_WIDTH, height: CreateFolderSheet.FORM_HEIGHT)
                case .rename(file: let file):
                    RenameFileSheet(homeState: homeState, id: file.id, name: file.name)
                        .frame(width: RenameFileSheet.FORM_WIDTH, height: RenameFileSheet.FORM_HEIGHT)
                case .share(file: let file):
                    ShareFileSheet(id: file.id, name: file.name, shares: file.shares)
                        .frame(width: ShareFileSheet.FORM_WIDTH, height: ShareFileSheet.FORM_HEIGHT)
                }
            }
        #endif
    }
}

extension View {
    func selectFolderSheets() -> some View {
        modifier(SelectFolderSheets())
    }
}


struct SelectFolderSheets: ViewModifier {
    @Environment(\.isConstrainedLayout) var isConstrainedLayout

    @EnvironmentObject var filesModel: FilesViewModel
    @EnvironmentObject var homeState: HomeState

    func body(content: Content) -> some View {
        #if os(iOS)
        if isConstrainedLayout {
            content
                .sheet(item: $homeState.selectSheetInfo) { action in
                    SelectFolderSheet(homeState: homeState, filesModel: filesModel, action: action)
                        .presentationDetents([.medium, .large])
                }

        } else {
            content
                .sheet(item: $homeState.selectSheetInfo) { action in
                    SelectFolderSheet(homeState: homeState, filesModel: filesModel, action: action)
                }
        }
        #else
        content
            .sheet(item: $homeState.selectSheetInfo) { action in
                SelectFolderSheet(homeState: homeState, filesModel: filesModel, action: action)
                    .frame(width: SelectFolderSheet.FORM_WIDTH, height: SelectFolderSheet.FORM_HEIGHT)
            }
        #endif
    }
}
