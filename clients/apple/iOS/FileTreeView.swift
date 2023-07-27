import SwiftUI
import SwiftLockbookCore

struct FileTreeView: View {
    @EnvironmentObject var sheets: SheetState
    @EnvironmentObject var currentDoc: DocumentService
    @EnvironmentObject var coreService: CoreService
    @EnvironmentObject var files: FileService
    @EnvironmentObject var onboarding: OnboardingService
    @EnvironmentObject var search: SearchService
    @EnvironmentObject var sync: SyncService
    @EnvironmentObject var share: ShareService
    
    @State var navigateToManageSub: Bool = false
    
    @State var searchInput: String = ""
    @State private var hideOutOfSpaceAlert = UserDefaults.standard.bool(forKey: "hideOutOfSpaceAlert")
    @State private var searchBar: UISearchBar?

    let currentFolder: File
    let account: Account
    
    var body: some View {
        VStack {
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
        }
        
        VStack {
            DocumentTabView(isiOS: true)
        }
        .toolbar {
            ToolbarItemGroup(placement: .navigationBarTrailing) {
                if let id = currentDoc.selectedDoc {
                    if let meta = DI.files.idsAndFiles[id] {
                        Button(action: {
                            exportFileAndShowShareSheet(meta: meta)
                               }, label: {
                            Label("Share externally to...", systemImage: "person.wave.2.fill")
                        })
                        .foregroundColor(.blue)
                        .padding(.trailing, 10)
                        
                        Button(action: {
                            DI.sheets.sharingFileInfo = meta
                               }, label: {
                            Label("Share", systemImage: "square.and.arrow.up.fill")
                        })
                        .foregroundColor(.blue)
                        .padding(.trailing, 5)
                    }
                }
                
                NavigationLink(
                    destination: PendingSharesView()) {
                        pendingShareToolbarIcon(isPendingSharesEmpty: share.pendingShares.isEmpty)
                            
                    }
                
                NavigationLink(
                    destination: SettingsView().equatable(), isActive: $onboarding.theyChoseToBackup) {
                        Image(systemName: "gearshape.fill")
                            .foregroundColor(.blue)
                            .padding(.trailing, 10)

                    }
            }
        }
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
