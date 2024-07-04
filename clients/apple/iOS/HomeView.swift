import SwiftUI
import SwiftWorkspace
import SwiftLockbookCore

struct HomeView: View {
    @EnvironmentObject var settings: SettingsService
    @EnvironmentObject var sync: SyncService
    @EnvironmentObject var workspace: WorkspaceState
    @EnvironmentObject var files: FileService
    @EnvironmentObject var share: ShareService
    @EnvironmentObject var billing: BillingService
    
    @State private var hideOutOfSpaceAlert = UserDefaults.standard.bool(forKey: "hideOutOfSpaceAlert")
    @State var searchInput: String = ""
    
    var body: some View {
        NavigationView {
            SidebarView(searchInput: $searchInput)
                .searchable(text: $searchInput, placement: .navigationBarDrawer(displayMode: .automatic), prompt: "Search")
            
            NavigationStack {
                workspaceView
                    .toolbar(.hidden, for: .navigationBar)
            }
            .modifier(AnimatedToolbarModifier(
                isSettingsViewShown: settings.showView,
                isManageSubscriptionsViewShown: billing.showManageSubscriptionView,
                isPendingSharesViewShown: share.showPendingSharesView,
                openDoc: workspace.openDoc)
            )
            
        }
        .alert(isPresented: Binding(get: { sync.outOfSpace && !hideOutOfSpaceAlert }, set: {_ in sync.outOfSpace = false })) {
            Alert(
                title: Text("Out of Space"),
                message: Text("You have run out of space!"),
                primaryButton: .default(Text("Upgrade now"), action: {
                    DI.billing.showManageSubscriptionView = true
                }),
                secondaryButton: .default(Text("Don't show me this again"), action: {
                    hideOutOfSpaceAlert = true
                    UserDefaults.standard.set(hideOutOfSpaceAlert, forKey: "hideOutOfSpaceAlert")
                })
            )
        }
    }
    
    var workspaceView: some View {
        WorkspaceView(DI.workspace, DI.coreService.corePtr)
            .equatable()
            .navigationDestination(isPresented: $settings.showView, destination: {
                SettingsView()
            })
            .navigationDestination(isPresented: $share.showPendingSharesView, destination: {
                PendingSharesView()
            })
            .navigationDestination(isPresented: $billing.showManageSubscriptionView, destination: {
                ManageSubscription()
            })
    }
}

struct AnimatedToolbarModifier: ViewModifier {
    var isSettingsViewShown: Bool
    var isManageSubscriptionsViewShown: Bool
    var isPendingSharesViewShown: Bool
    var openDoc: UUID?
    
    @State private var isToolbarVisible: Bool = true

    func body(content: Content) -> some View {
        content
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                if let id = openDoc, let meta = DI.files.idsAndFiles[id] {
                    Button(action: {
                        content.exportFileAndShowShareSheet(meta: meta)
                    }) {
                        Label("Share externally to...", systemImage: "square.and.arrow.up.fill")
                    }
                    .foregroundColor(.blue)
                    .padding(.trailing, 5)
                    
                    Button(action: {
                        DI.sheets.sharingFileInfo = meta
                    }) {
                        Label("Share", systemImage: "person.wave.2.fill")
                    }
                    .foregroundColor(.blue)
                    .padding(.trailing, 5)
                }
            }
            .toolbar(isToolbarVisible ? .visible : .hidden, for: .navigationBar)
            .onChange(of: isSettingsViewShown) { _ in animateToolbar() }
            .onChange(of: isManageSubscriptionsViewShown) { _ in animateToolbar() }
            .onChange(of: isPendingSharesViewShown) { _ in animateToolbar() }
    }

    private func animateToolbar() {
        isToolbarVisible = isSettingsViewShown || isManageSubscriptionsViewShown || isPendingSharesViewShown
    }
}

struct SidebarView: View {
    @EnvironmentObject var files: FileService
    @EnvironmentObject var search: SearchService
    
    @EnvironmentObject var share: ShareService
    @EnvironmentObject var billing: BillingService
    @EnvironmentObject var settings: SettingsService
    
    @State private var searchBar: UISearchBar?
    
    @Environment(\.dismissSearch) private var dismissSearch
    @Environment(\.isSearching) var isSearching
    
    @Binding var searchInput: String
    
    var body: some View {
        Group {
            if search.isPathAndContentSearching {
                if !search.isPathAndContentSearchInProgress && !search.pathAndContentSearchQuery.isEmpty && search.pathAndContentSearchResults.isEmpty {
                    noSearchResultsView
                } else {
                    ScrollView {
                        if search.isPathAndContentSearchInProgress {
                            ProgressView()
                                .frame(width: 20, height: 20)
                                .padding(.top)
                        }
                        
                        if !search.pathAndContentSearchResults.isEmpty {
                            searchResultsView
                        }
                    }
                }
            } else {
                suggestedAndFilesView
            }
        }
        .onChange(of: searchInput) { newInput in
            DI.search.search(query: newInput, isPathAndContentSearch: true)
        }
        .onChange(of: isSearching, perform: { newInput in
            if newInput {
                DI.search.startSearchThread(isPathAndContentSearch: true)
            } else {
                DI.search.endSearch(isPathAndContentSearch: true)
            }
        })
        .introspectNavigationController { nav in
            searchBar = nav.navigationBar.subviews.first { view in
                view is UISearchBar
            } as? UISearchBar
        }
        .background(
            Button("Search Paths And Content") {
                focusSearchBar()
            }
            .keyboardShortcut("f", modifiers: [.command, .shift])
            .hidden()
        )
        .navigationBarTitle(DI.accounts.account?.username ?? "...")
    }
    
    var noSearchResultsView: some View {
        VStack {
            Text("No results.")
                .font(.headline)
                .foregroundColor(.gray)
                .fontWeight(.bold)
                .padding()
            
            Spacer()
        }
    }
    
    var searchResultsView: some View {
        ForEach(search.pathAndContentSearchResults) { result in
            switch result {
            case .PathMatch(_, let meta, let name, let path, let matchedIndices, _):
                Button(action: {
                    DI.workspace.requestOpenDoc(meta.id)
                    DI.files.intoChildDirectory(meta)
                    dismissSearch()
                }) {
                    SearchFilePathCell(name: name, path: path, matchedIndices: matchedIndices)
                }
                .padding(.horizontal)

            case .ContentMatch(_, let meta, let name, let path, let paragraph, let matchedIndices, _):
                Button(action: {
                    DI.workspace.requestOpenDoc(meta.id)
                    DI.files.intoChildDirectory(meta)
                    dismissSearch()
                }) {
                    SearchFileContentCell(name: name, path: path, paragraph: paragraph, matchedIndices: matchedIndices)
                }
                .padding(.horizontal)
            }
            
            Divider()
                .padding(.leading, 20)
                .padding(.vertical, 5)
        }
    }
    
    var suggestedAndFilesView: some View {
        VStack {
            VStack(alignment: .leading) {
                SuggestedDocs(isiOS: false)
                
                Text("Files")
                    .bold()
                    .foregroundColor(.primary)
                    .textCase(.none)
                    .font(.headline)
                    .padding(.top)
                    .padding(.bottom, 5)
                
                if let root = files.root {
                    OutlineSection(root: root)
                } else {
                    ProgressView()
                        .padding(.leading)
                }
            }
            .padding(.horizontal)
            
            Spacer()
            
            BottomBar()
        }
        .toolbar {
            ToolbarItemGroup(placement: .topBarTrailing) {
                Button(action: {
                    DI.share.showPendingSharesView = true
                }) {
                    pendingShareToolbarIcon(isPendingSharesEmpty: DI.share.pendingShares?.isEmpty ?? false)
                }

                Button(action: {
                    DI.settings.showView = true

                }) {
                    Image(systemName: "gearshape.fill")
                        .foregroundColor(.blue)
                        .padding(.trailing, 10)
                }
            }
        }
    }
    
    func focusSearchBar() {
        searchBar?.becomeFirstResponder()
    }
}
