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
                    .ignoresSafeArea(.keyboard)
            }
        }
        .overlay(alignment: .topLeading) {
            if horizontalSizeClass == .regular, homeState.sidebarState == .closed {
                Button {
                    withAnimation {
                        homeState.sidebarState = .open
                    }
                } label: {
                    Image(systemName: "sidebar.left")
                        .imageScale(.large)
                        .foregroundStyle(.primary)
                }
                .buttonStyle(.borderless)
                .frame(
                    width: iOSMTK.SIDEBAR_TOGGLE_INSET,
                    height: iOSMTK.TAB_BAR_HEIGHT
                )
            }
        }
        .toolbar(
            horizontalSizeClass == .regular ? .hidden : .automatic,
            for: .navigationBar
        )
        .toolbar {
            if horizontalSizeClass == .compact, workspaceOutput.tabCount > 0 {
                ToolbarItem(placement: .topBarTrailing) {
                    Button {
                        showTabsSheet()
                    } label: {
                        ZStack(alignment: .center) {
                            RoundedRectangle(cornerSize: .init(width: 4, height: 4))
                                .stroke(lineWidth: 2)
                                .frame(width: 20, height: 20)

                            Text(workspaceOutput.tabCount < 100 ? String(workspaceOutput.tabCount) : ":D")
                                .font(.footnote)
                        }
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
        .onAppear {
            workspaceInput.sidebarVisible = homeState.sidebarState == .open
        }
        .onChange(of: homeState.sidebarState) { newValue in
            workspaceInput.sidebarVisible = newValue == .open
            workspaceInput.redraw.send(())
        }
    }

    func showTabsSheet() {
        homeState.tabsSheetInfo = TabSheetInfo(
            info: workspaceInput.getTabsIds().map { id in
                guard let file = filesModel.idsToFiles[id] else {
                    return nil
                }

                return (name: file.name, id: file.id)
            }.compactMap { $0 }
        )
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
                        ToolbarSpacer(.fixed, placement: .topBarLeading)

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
    NavigationStack {
        DetailView()
            .withCommonPreviewEnvironment()
    }
}
