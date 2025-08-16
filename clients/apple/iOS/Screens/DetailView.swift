import SwiftUI
import SwiftWorkspace

struct DetailView: View {
    @Environment(\.isPreview) var isPreview
    @Environment(\.isConstrainedLayout) var isConstrainedLayout

    @EnvironmentObject var workspaceState: WorkspaceState
    @EnvironmentObject var homeState: HomeState
    @EnvironmentObject var filesModel: FilesViewModel
    
    @State var sheetHeight: CGFloat = 0

    var body: some View {
        Group {
            if isPreview {
                Text("This is a preview.")
            } else {
                WorkspaceView(AppState.workspaceState, AppState.lb.lbUnsafeRawPtr)
                    .modifier(OnLbLinkViewModifier())
            }
        }
        .onAppear {
            toggleTabVisibility()
        }
        .toolbar {
            ToolbarItemGroup(placement: .topBarTrailing) {
                HStack(alignment: .lastTextBaseline, spacing: 5) {
                    if workspaceState.openDoc != nil {
                        Button(action: {
                            runOnOpenDoc { file in
                                homeState.sheetInfo = .share(file: file)
                            }
                        }, label: {
                            Image(systemName: "person.wave.2.fill")
                        })
                        
                        Button(action: {
                            runOnOpenDoc { file in
                                exportFiles(homeState: homeState, files: [file])
                            }
                        }, label: {
                            Image(systemName: "square.and.arrow.up.fill")
                        })
                    }
                        
                    if isConstrainedLayout && workspaceState.tabCount > 0 {
                        Button(action: {
                            self.showTabsSheet()
                        }, label: {
                            ZStack(alignment: .center) {
                                RoundedRectangle(cornerSize: .init(width: 4, height: 4))
                                    .stroke(Color.accentColor, lineWidth: 2)
                                    .frame(width: 20, height: 20)
                                    
                                Text(workspaceState.tabCount < 100 ? String(workspaceState.tabCount) : ":D")
                                    .font(.footnote)
                                    .foregroundColor(.accentColor)
                            }
                        })
                    }
                }
            }
        }
        .optimizedSheet(item: $homeState.tabsSheetInfo, constrainedSheetHeight: $sheetHeight) { info in
            TabsSheet(info: info.info)
        }
        .fileOpSheets(constrainedSheetHeight: $sheetHeight)
        .modifier(ConstrainedTitle())
    }
    
    func showTabsSheet() {
        homeState.tabsSheetInfo = TabSheetInfo(info: workspaceState.getTabsIds().map({ id in
            guard let file = filesModel.idsToFiles[id] else {
                return nil
            }
            
            return (name: file.name, id: file.id)
        }).compactMap({ $0 }))
    }
    
    
    func toggleTabVisibility() {
        DispatchQueue.main.async {
            AppState.workspaceState.showTabs = !isConstrainedLayout
        }
    }
    
    func runOnOpenDoc(f: @escaping (File) -> Void) {
        guard let id = AppState.workspaceState.openDoc else {
            return
        }
        
        if let file = filesModel.idsToFiles[id] {
            f(file)
        }
    }

}

struct ConstrainedTitle: ViewModifier {
    @EnvironmentObject var workspaceState: WorkspaceState
    @EnvironmentObject var filesModel: FilesViewModel
    
    @Environment(\.isConstrainedLayout) var isConstrainedLayout

    var title: String {
        get {
            guard let id = workspaceState.openDoc else {
                return ""
            }
            
            return filesModel.idsToFiles[id]?.name ?? "Unknown file"
        }
    }
    
    func body(content: Content) -> some View {
        if isConstrainedLayout {
            content
                .toolbar {
                    ToolbarItem(placement: .topBarLeading) {
                        Button(action: {
                            workspaceState.renameOpenDoc = true
                        }, label: {
                            Text(title)
                                .foregroundStyle(.foreground)
                                .lineLimit(1)
                                .truncationMode(.tail)
                                .frame(width: 200, alignment: .leading)
                        })
                    }
                }
        } else {
            content
        }
    }
}

#Preview {
    let workspaceState = WorkspaceState()
    workspaceState.tabCount = 5
    
    return NavigationStack {
        DetailView()
            .environmentObject(workspaceState)
            .environmentObject(HomeState())
    }
}
