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
    @EnvironmentObject var files: FileService    // todo: is it a problem if this reloads? 
        
    var body: some View {
        ZStack {
            if files.pendingSharesOpen {
                PendingSharesView()
            } else {
                WorkspaceView(DI.workspace, get_core_ptr()).equatable()
            }
            
        }
//        .toolbar {
//            ToolbarItemGroup {
//                if let id = files.workspace.openDoc,
//                   let meta = files.idsAndFiles[id],
//                   !files.pendingSharesOpen {
//                    
//                    let view = MacOSShareSpaceHolder()
//                    
//                    ZStack {
//                        view.id(UUID())
//                        
//                        Button(action: {
//                            view.view.exportFileAndShowShareSheet(meta: meta)
//                        }, label: {
//                            Label("Share externally to...", systemImage: "square.and.arrow.up.fill")
//                                .imageScale(.large)
//                        })
//                        .foregroundColor(.blue)
//                        .padding(.trailing, 10)
//                    }
//                    
//                    Button(action: {
//                        DI.sheets.sharingFileInfo = meta
//                    }, label: {
//                        Label("Share", systemImage: "person.wave.2.fill")
//                            .imageScale(.large)
//                    })
//                    .foregroundColor(.blue)
//                    .padding(.trailing, 5)
//                }
//                
//                Button(action: {
//                    files.pendingSharesOpen.toggle()
//                }) {
//                    pendingShareToolbarIcon(isPendingSharesEmpty: share.pendingShares.isEmpty)
//                        .imageScale(.large)
//                }
//            }
//        }
    }
}

struct MacOSShareSpaceHolder: NSViewRepresentable {
    let view = NSView()
        
    func makeNSView(context: Context) -> NSView {
        view
    }

    func updateNSView(_ nsView: NSView, context: Context) {}
}
