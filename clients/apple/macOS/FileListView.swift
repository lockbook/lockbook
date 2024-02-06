import SwiftUI
import SwiftLockbookCore
import DSFQuickActionBar
import SwiftWorkspace
import CLockbookCore

struct FileListView: View {
    @State var searchInput: String = ""
    @State var expandedFolders: [File] = []
    @State var lastOpenDoc: File? = nil
    
    @State var treeBranchState: Bool = true
        
    var body: some View {
        VStack {
            SearchWrapperView(
                searchInput: $searchInput,
                mainView: mainView,
                isiOS: false)
            .searchable(text: $searchInput, prompt: "Search")
                
            BottomBar()
        }
            
        DetailView()
    }
    
    var mainView: some View {
        VStack {
            SuggestedDocs()

            fileTreeView
        }
    }
    
    var fileTreeView: some View {
        Group {
            Button(action: {
                withAnimation {
                    treeBranchState.toggle()
                }
            }) {
                HStack {
                    Text("Files")
                        .bold()
                        .foregroundColor(.gray)
                        .font(.subheadline)
                    Spacer()
                    if treeBranchState {
                        Image(systemName: "chevron.down")
                            .foregroundColor(.gray)
                            .imageScale(.small)
                    } else {
                        Image(systemName: "chevron.right")
                            .foregroundColor(.gray)
                            .imageScale(.small)
                    }
                }
                .padding(.top)
                .padding(.horizontal)
                .contentShape(Rectangle())
            }
            
            if treeBranchState {
                FileTreeView(expandedFolders: $expandedFolders, lastOpenDoc: $lastOpenDoc)
                    .padding(.leading, 4)
                Spacer()
            } else {
                Spacer()
            }
        }
    }
}

struct DetailView: View {
    @EnvironmentObject var search: SearchService
    @EnvironmentObject var share: ShareService
    @EnvironmentObject var workspace: WorkspaceState
        
    var body: some View {
        ZStack {
            WorkspaceView(DI.workspace, get_core_ptr())
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
