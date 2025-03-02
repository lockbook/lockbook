import SwiftUI
import SwiftWorkspace

struct DetailView: View {
    @Environment(\.isPreview) var isPreview

    @EnvironmentObject var workspaceState: WorkspaceState
    @EnvironmentObject var homeState: HomeState
    
    @State var sheetHeight: CGFloat = 0

    var body: some View {
        Group {
            if isPreview {
                Text("This is a preview.")
            } else {
                WorkspaceView(workspaceState, AppState.lb.lbUnsafeRawPtr)
            }
        }
        .toolbar {
            ToolbarItemGroup(placement: .topBarTrailing) {
                HStack(alignment: .bottom, spacing: 5) {
                    if workspaceState.openDoc != nil {
                        Button(action: {
                            self.runOnOpenDoc { file in
                                homeState.sheetInfo = .share(file: file)
                            }
                        }, label: {
                            Image(systemName: "person.wave.2.fill")
                        })
                        
                        Button(action: {
                            self.runOnOpenDoc { file in
                                exportFiles(homeState: homeState, files: [file])
                            }
                        }, label: {
                            Image(systemName: "square.and.arrow.up.fill")
                        })
                        
                        if workspaceState.openTabs > 1 {
                            Button(action: {
                                self.showTabsSheet()
                            }, label: {
                                ZStack {
                                    Image(systemName: "rectangle.fill")
                                    
                                    Text(workspaceState.openTabs < 100 ? String(workspaceState.openTabs) : ":D")
                                        .font(.callout)
                                        .foregroundColor(.white)
                                }
                            })
                        }
                    }
                }
            }
        }
        .optimizedSheet(item: $homeState.tabsSheetInfo, constrainedSheetHeight: $sheetHeight) { info in
            TabsSheet(info: info.info)
        }
        .fileOpSheets(workspaceState: workspaceState, constrainedSheetHeight: $sheetHeight)
        .modifier(ConstrainedTitle())
    }
    
    func showTabsSheet() {
            homeState.tabsSheetInfo = TabSheetInfo(info: workspaceState.getTabsIds().map({ id in
            switch AppState.lb.getFile(id: id) {
            case .success(let file):
                return (name: file.name, id: file.id)
            case .failure(_):
                return nil
            }
        }).compactMap({ $0 }))
    }
    
    func runOnOpenDoc(f: @escaping (File) -> Void) {
        guard let id = workspaceState.openDoc else {
            return
        }
        
        if let file =  try? AppState.lb.getFile(id: id).get() {
            f(file)
        }
    }
}

struct ConstrainedTitle: ViewModifier {
    @EnvironmentObject var workspaceState: WorkspaceState
    @Environment(\.isConstrainedLayout) var isConstrainedLayout

    var title: String {
        get {
            guard let id = workspaceState.openDoc else {
                return ""
            }
            
            return (try? AppState.lb.getFile(id: id).get())?.name ?? "unknown file"
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
    workspaceState.openTabs = 5
    
    return NavigationStack {
        DetailView()
            .environmentObject(workspaceState)
            .environmentObject(HomeState(workspaceState: workspaceState))
    }
}
