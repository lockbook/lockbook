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
                WorkspaceView(workspaceInput, workspaceOutput, AppState.lb.lbUnsafeRawPtr)
                    .modifier(OnLbLinkViewModifier())
            }
        }
        .toolbar {
            ToolbarItemGroup(placement: .topBarTrailing) {
                HStack(alignment: .lastTextBaseline, spacing: 5) {
                    if workspaceOutput.openDoc != nil {
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
                        
                    if horizontalSizeClass == .compact && workspaceOutput.tabCount > 0 {
                        Button(action: {
                            self.showTabsSheet()
                        }, label: {
                            ZStack(alignment: .center) {
                                RoundedRectangle(cornerSize: .init(width: 4, height: 4))
                                    .stroke(Color.accentColor, lineWidth: 2)
                                    .frame(width: 20, height: 20)
                                    
                                Text(workspaceOutput.tabCount < 100 ? String(workspaceOutput.tabCount) : ":D")
                                    .font(.footnote)
                                    .foregroundColor(.accentColor)
                            }
                        })
                    }
                }
            }
        }
        .optimizedSheet(item: $homeState.tabsSheetInfo, compactSheetHeight: $sheetHeight) { info in
            TabsSheet(info: info.info)
        }
        .fileOpSheets(compactSheetHeight: $sheetHeight)
        .modifier(CompactTitle())
    }
    
    func showTabsSheet() {
        homeState.tabsSheetInfo = TabSheetInfo(info: workspaceInput.getTabsIds().map({ id in
            guard let file = filesModel.idsToFiles[id] else {
                return nil
            }
            
            return (name: file.name, id: file.id)
        }).compactMap({ $0 }))
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
        get {
            guard let id = workspaceOutput.openDoc else { return "" }
            return filesModel.idsToFiles[id]?.name ?? "Unknown file"
        }
    }
    
    func body(content: Content) -> some View {
        if horizontalSizeClass == .compact {
            content
                .toolbar {
                    ToolbarItem(placement: .topBarLeading) {
                        Button(action: {
                            openRenameSheet()
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
    let workspaceOutput = WorkspaceOutputState()
    workspaceOutput.tabCount = 5
    
    return NavigationStack {
        DetailView()
            .environmentObject(HomeState(workspaceOutput: WorkspaceOutputState(), filesModel: FilesViewModel()))
            .environmentObject(FilesViewModel())
            .environmentObject(WorkspaceInputState())
            .environmentObject(WorkspaceOutputState())
    }
}
