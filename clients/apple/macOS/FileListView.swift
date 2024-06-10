import SwiftUI
import SwiftLockbookCore
import DSFQuickActionBar
import SwiftWorkspace

struct FileListView: View {
    @State var searchInput: String = ""
        
    var body: some View {
        VStack {
//            SearchWrapperView(
//                searchInput: $searchInput,
//                mainView: EmptyView(),
//                isiOS: false)
//            .searchable(text: $searchInput, prompt: "Search")
            
//            Text("The search is \(searchInput)")
//                .searchable(text: $searchInput, prompt: "Search")
//                .onAppear {
//                    DI.search.startSearchThread(isPathAndContentSearch: true)
//                }
//                .onChange(of: searchInput) { newInput in
//                    print("making a search: \"\(newInput)\"")
//                    DI.search.search(query: newInput, isPathAndContentSearch: true)
//                }

            BottomBar()
        }
            
        DetailView()
    }
}

struct DetailView: View {
    @EnvironmentObject var search: SearchService
    @EnvironmentObject var share: ShareService
    @EnvironmentObject var workspace: WorkspaceState
        
    var body: some View {
        ZStack {
            WorkspaceView(DI.workspace, DI.coreService.corePtr)
                .equatable()
                .opacity(workspace.pendingSharesOpen ? 0.0 : 1.0)
            
            if workspace.pendingSharesOpen {
                PendingSharesView()
            }
        }
        .toolbar {
            ToolbarItemGroup {
                if let id = workspace.openDoc,
                   let meta = DI.files.idsAndFiles[id],
                   !workspace.pendingSharesOpen {
                    ZStack {
                        Button(action: {
                            NSApp.keyWindow?.toolbar?.items.first?.view?.exportFileAndShowShareSheet(meta: meta)
                        }, label: {
                            Label("Share externally to...", systemImage: "square.and.arrow.up.fill")
                                .imageScale(.large)
                        })
                        .foregroundColor(.blue)
                        .padding(.trailing, 10)
                    }
                    
                    Button(action: {
                        DI.sheets.sharingFileInfo = meta
                    }, label: {
                        Label("Share", systemImage: "person.wave.2.fill")
                            .imageScale(.large)
                    })
                    .foregroundColor(.blue)
                    .padding(.trailing, 5)
                }
                
                Button(action: {
                    DI.workspace.pendingSharesOpen.toggle()
                }) {
                    pendingShareToolbarIcon(isPendingSharesEmpty: share.pendingShares?.isEmpty ?? true)
                        .imageScale(.large)
                }
            }
        }
    }
}
