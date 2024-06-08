import SwiftUI
import SwiftWorkspace
import SwiftLockbookCore
import CLockbookCore

struct FileTreeView: View {
    @EnvironmentObject var sheets: SheetState
    @EnvironmentObject var coreService: CoreService
    @EnvironmentObject var files: FileService
    @EnvironmentObject var onboarding: OnboardingService
    @EnvironmentObject var sync: SyncService
    @EnvironmentObject var share: ShareService
    @EnvironmentObject var workspace: WorkspaceState
    
    @State var navigateToManageSub: Bool = false
    
    @State var searchInput: String = ""
    @State private var hideOutOfSpaceAlert = UserDefaults.standard.bool(forKey: "hideOutOfSpaceAlert")
    @State private var searchBar: UISearchBar?

    let currentFolder: File
    let account: Account
    
    var body: some View {
        SearchWrapperView(
            searchInput: $searchInput,
            mainView: mainView,
            isiOS: false)
        .searchable(text: $searchInput, placement: .navigationBarDrawer(displayMode: .automatic), prompt: "Search")
        .background(
            Button("Search Paths And Content") {
                focusSearchBar()
            }
            .keyboardShortcut("f", modifiers: [.command, .shift])
            .hidden()
        )
        .introspectNavigationController { nav in
            searchBar = nav.navigationBar.subviews.first { view in
                view is UISearchBar
            } as? UISearchBar
        }
    
        WorkspaceView(DI.workspace, DI.coreService.corePtr)
            .equatable()
            .workspaceToolbar(theyChoseToBackup: $onboarding.theyChoseToBackup)
            .alert(isPresented: Binding(get: { sync.outOfSpace && !hideOutOfSpaceAlert }, set: {_ in sync.outOfSpace = false })) {
                Alert(
                    title: Text("Out of Space"),
                    message: Text("You have run out of space!"),
                    primaryButton: .default(Text("Upgrade now"), action: {
                        navigateToManageSub = true
                    }),
                    secondaryButton: .default(Text("Don't show me this again"), action: {
                        hideOutOfSpaceAlert = true
                        UserDefaults.standard.set(hideOutOfSpaceAlert, forKey: "hideOutOfSpaceAlert")
                    })
                )
            }
            .background(
                NavigationLink(destination: ManageSubscription(), isActive: $navigateToManageSub, label: {
                    EmptyView()
                })
                .hidden()
            )
    }
    
    var mainView: some View {
        Group {
            VStack(alignment: .leading) {
                SuggestedDocs(isiOS: false)
                
                Text("Files")
                    .bold()
                    .foregroundColor(.primary)
                    .textCase(.none)
                    .font(.headline)
                    .padding(.top)
                    .padding(.bottom, 5)
                
                OutlineSection(root: currentFolder)
            }
            .padding(.horizontal)
            
            BottomBar()
        }
    }
    
    func focusSearchBar() {
        searchBar?.becomeFirstResponder()
    }
}

extension View {
    func workspaceToolbar(theyChoseToBackup: Binding<Bool>) -> some View {
        let basicToolbarItems = Group {
            NavigationLink(
                destination: PendingSharesView()) {
                    pendingShareToolbarIcon(isPendingSharesEmpty: DI.share.pendingShares?.isEmpty ?? false)
                }
            
            NavigationLink(
                destination: SettingsView().equatable(), isActive: theyChoseToBackup) {
                    Image(systemName: "gearshape.fill")
                        .foregroundColor(.blue)
                        .padding(.trailing, 10)
                }
        }
        
        return self.toolbar {
            if let id = DI.workspace.openDoc, let meta = DI.files.idsAndFiles[id] {
                ToolbarItemGroup(placement: .topBarTrailing) {
                    Button(action: {
                        exportFileAndShowShareSheet(meta: meta)
                    }, label: {
                        Label("Share externally to...", systemImage: "square.and.arrow.up.fill")
                    })
                    .foregroundColor(.blue)
                    .padding(.trailing, 5)
                    
                    Button(action: {
                        DI.sheets.sharingFileInfo = meta
                    }, label: {
                        Label("Share", systemImage: "person.wave.2.fill")
                    })
                    .foregroundColor(.blue)
                    .padding(.trailing, 5)
                    
                    basicToolbarItems
                }
            } else {
                ToolbarItemGroup {
                    basicToolbarItems
                }
            }
        }
    }
}
