import SwiftUI
import SwiftWorkspace

extension View {
    func fileOpSheets(
        compactSheetHeight: Binding<CGFloat>
    ) -> some View {
        modifier(FileOpSheets(compactSheetHeight: compactSheetHeight))
    }
}

struct FileOpSheets: ViewModifier {
    @Environment(\.horizontalSizeClass) var horizontalSizeClass
    @EnvironmentObject var homeState: HomeState
    @EnvironmentObject var workspaceInput: WorkspaceInputState

    @Binding var compactSheetHeight: CGFloat

    func body(content: Content) -> some View {
        // A little bit odd but not too bad
        #if os(iOS)
            if horizontalSizeClass == .compact {
                content
                    .sheet(item: $homeState.sheetInfo) { info in
                        Group {
                            switch info {
                            case .createFolder(let parent):
                                CreateFolderSheet(
                                    homeState: homeState,
                                    parentId: parent.id,
                                    showExitButton: false
                                )
                                .autoSizeSheet(sheetHeight: $compactSheetHeight)
                            case .rename(let file):
                                RenameFileSheet(
                                    homeState: homeState,
                                    id: file.id,
                                    name: file.name,
                                    showExitButton: false
                                )
                                .autoSizeSheet(sheetHeight: $compactSheetHeight)
                            case .share(let file):
                                ShareFileSheet(
                                    id: file.id,
                                    name: file.name,
                                    shares: file.shares,
                                    showExitButton: false
                                )
                                .autoSizeSheet(sheetHeight: $compactSheetHeight)
                            case .importPicker:
                                ImportFilePicker()
                            }
                        }
                        .environmentObject(workspaceInput)
                    }
            } else {
                content
                    .formSheet(item: $homeState.sheetInfo) { info in
                        Group {
                            switch info {
                            case .createFolder(let parent):
                                CreateFolderSheet(
                                    homeState: homeState,
                                    parentId: parent.id,
                                    showExitButton: true
                                )
                                .frame(
                                    width: CreateFolderSheet.FORM_WIDTH,
                                    height: CreateFolderSheet.FORM_HEIGHT
                                )
                            case .rename(let file):
                                RenameFileSheet(
                                    homeState: homeState,
                                    id: file.id,
                                    name: file.name,
                                    showExitButton: true
                                )
                                .frame(
                                    width: RenameFileSheet.FORM_WIDTH,
                                    height: RenameFileSheet.FORM_HEIGHT
                                )
                            case .share(let file):
                                ShareFileSheet(
                                    id: file.id,
                                    name: file.name,
                                    shares: file.shares,
                                    showExitButton: true
                                )
                                .frame(
                                    width: ShareFileSheet.FORM_WIDTH,
                                    height: ShareFileSheet.FORM_HEIGHT
                                )
                            case .importPicker:
                                ImportFilePicker()
                            }
                        }
                        .environmentObject(workspaceInput)
                    }
            }
        #else
            content
                .sheet(item: $homeState.sheetInfo) { info in
                    Group {
                        switch info {
                        case .createFolder(let parent):
                            CreateFolderSheet(
                                homeState: homeState,
                                parentId: parent.id,
                                showExitButton: true
                            )
                            .frame(
                                width: CreateFolderSheet.FORM_WIDTH,
                                height: CreateFolderSheet.FORM_HEIGHT
                            )
                        case .rename(let file):
                            RenameFileSheet(
                                homeState: homeState,
                                id: file.id,
                                name: file.name,
                                showExitButton: true
                            )
                            .frame(
                                width: RenameFileSheet.FORM_WIDTH,
                                height: RenameFileSheet.FORM_HEIGHT
                            )
                        case .share(let file):
                            ShareFileSheet(
                                id: file.id,
                                name: file.name,
                                shares: file.shares,
                                showExitButton: true
                            )
                            .frame(
                                width: ShareFileSheet.FORM_WIDTH,
                                height: ShareFileSheet.FORM_HEIGHT
                            )
                        case .importPicker:
                            // Unused
                            EmptyView()
                        }
                    }
                    .environmentObject(workspaceInput)
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
    @Environment(\.horizontalSizeClass) var horizontalSizeClass

    @EnvironmentObject var filesModel: FilesViewModel
    @EnvironmentObject var homeState: HomeState

    func body(content: Content) -> some View {
        #if os(iOS)
            if horizontalSizeClass == .compact {
                content
                    .sheet(item: $homeState.selectSheetInfo) { action in
                        SelectFolderSheet(
                            homeState: homeState,
                            filesModel: filesModel,
                            action: action,
                            showExitButton: false
                        )
                        .presentationDetents([.medium, .large])
                    }

            } else {
                content
                    .sheet(item: $homeState.selectSheetInfo) { action in
                        SelectFolderSheet(
                            homeState: homeState,
                            filesModel: filesModel,
                            action: action,
                            showExitButton: false
                        )
                    }
            }
        #else
            content
                .sheet(item: $homeState.selectSheetInfo) { action in
                    SelectFolderSheet(
                        homeState: homeState,
                        filesModel: filesModel,
                        action: action,
                        showExitButton: true
                    )
                    .frame(
                        width: SelectFolderSheet.FORM_WIDTH,
                        height: SelectFolderSheet.FORM_HEIGHT
                    )
                }
        #endif
    }
}
