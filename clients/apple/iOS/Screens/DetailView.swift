import SwiftUI
import SwiftWorkspace

struct DetailView: View {
    @Environment(\.isPreview) var isPreview
    @Environment(\.horizontalSizeClass) var horizontalSizeClass

    @EnvironmentObject var workspaceInput: WorkspaceInputState
    @EnvironmentObject var workspaceOutput: WorkspaceOutputState
    
    @EnvironmentObject var homeState: HomeState
    @EnvironmentObject var filesModel: FilesViewModel
            
    @State var sheetHeight: CGFloat = 0
    
    var body: some View {
        Group {
            if isPreview {
                Text("This is a preview.")
            } else {
                WorkspaceView()
                    .modifier(OnLbLinkViewModifier())
            }
        }
        .toolbar {
            if workspaceOutput.openDoc != nil {
                ToolbarItem(placement: .topBarTrailing) {
                    Button {
                        runOnOpenDoc { file in
                            homeState.sheetInfo = .share(file: file)
                        }
                    } label: {
                        Label("Share", systemImage: "person.wave.2.fill")
                    }
                }

                ToolbarItem(placement: .topBarTrailing) {
                    Button {
                        runOnOpenDoc { file in
                            exportFiles(homeState: homeState, files: [file])
                        }
                    } label: {
                        Label("Export", systemImage: "square.and.arrow.up.fill")
                    }
                }
            }

            if horizontalSizeClass == .compact && workspaceOutput.tabCount > 0 {
                ToolbarItem(placement: .topBarTrailing) {
                    Button {
                        self.showTabsSheet()
                    } label: {
                        Label(
                            "\(workspaceOutput.tabCount) Tabs",
                            systemImage: "square.on.square"
                        )
                        .labelStyle(.iconOnly)
                    }
                }
            }
        }
        .optimizedSheet(
            item: $homeState.tabsSheetInfo,
            compactSheetHeight: $sheetHeight
        ) { info in
            TabsSheet(info: info.info)
        }
        .fileOpSheets(compactSheetHeight: $sheetHeight)
        .modifier(CompactTitle())
    }

    func showTabsSheet() {
        homeState.tabsSheetInfo = TabSheetInfo(
            info: workspaceInput.getTabsIds().map({ id in
                guard let file = filesModel.idsToFiles[id] else {
                    return nil
                }

                return (name: file.name, id: file.id)
            }).compactMap({ $0 })
        )
    }

    func runOnOpenDoc(f: @escaping (File) -> Void) {
        guard let id = workspaceOutput.openDoc else {
            return
        }

        if let file = filesModel.idsToFiles[id] {
            f(file)
        }
    }

}

struct CompactTitle: ViewModifier {
    @EnvironmentObject var homeState: HomeState
    @EnvironmentObject var workspaceOutput: WorkspaceOutputState
    @EnvironmentObject var filesModel: FilesViewModel

    @Environment(\.horizontalSizeClass) var horizontalSizeClass

    var title: String {
        guard let id = workspaceOutput.openDoc else { return "" }
        return filesModel.idsToFiles[id]?.name ?? "Unknown file"
    }

    func body(content: Content) -> some View {
        if horizontalSizeClass == .compact {
            content
                .toolbar {
                    if workspaceOutput.openDoc != nil {
                        ToolbarItem(placement: .topBarLeading) {
                            Button(
                                action: {
                                    openRenameSheet()
                                },
                                label: {
                                    Text(title)
                                        .foregroundStyle(.foreground)
                                        .lineLimit(1)
                                        .truncationMode(.tail)
                                        .frame(width: 200, alignment: .leading)
                                }
                            )
                        }
                    }
                }
        } else {
            content
        }
    }
    func openRenameSheet() {
        guard let id = workspaceOutput.openDoc else {
            return
        }

        guard let file = filesModel.idsToFiles[id] else {
            return
        }

        DispatchQueue.main.async {
            homeState.sheetInfo = .rename(file: file)
        }
    }
}

#Preview {
    return NavigationStack {
        DetailView()
            .withCommonPreviewEnvironment()
    }
}
