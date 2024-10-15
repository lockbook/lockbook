import SwiftUI
import SwiftWorkspace
import SwiftLockbookCore
import Foundation

struct ConstrainedHomeViewWrapper: View {
    @EnvironmentObject var workspace: WorkspaceState
    @EnvironmentObject var files: FileService
    @EnvironmentObject var search: SearchService
    @EnvironmentObject var settings: SettingsService
    @EnvironmentObject var billing: BillingService
    @EnvironmentObject var share: ShareService
    
    @State var searchInput: String = ""
    
    var canShowFileTreeInfo: Bool {
        get {
            files.path.last?.fileType != .Document && !settings.showView && !share.showPendingSharesView && !billing.showManageSubscriptionView && !search.isPathAndContentSearching
        }
    }
    
    var body: some View {
        ZStack {
            VStack {
                NavigationStack(path: $files.path) {
                    mainView
                }
            }
            .onChange(of: files.path) { new in
                if files.path.last?.fileType != .Document && DI.workspace.openDoc != nil {
                    DI.workspace.requestCloseAllTabs()
                }
            }
            .ignoresSafeArea(.container, edges: [.bottom])
            
            if canShowFileTreeInfo {
                VStack {
                    Spacer()
                    
                    VStack(spacing: 0) {
                        FilePathBreadcrumb()
                        
                        BottomBar(isiOS: true)
                            .padding(.top, 10)
                    }
                    .background(
                        EmptyView()
                            .background(.background)
                            .ignoresSafeArea(.container, edges: [.bottom])
                    )
                }
            }
            
            if files.path.last?.fileType != .Document {
                WorkspaceView(DI.workspace, DI.coreService.corePtr)
                    .id(workspace.workspaceViewId)
                    .opacity(0)
            }
        }
    }
    
    var mainView: some View {
        ConstrainedHomeView(searchInput: $searchInput)
            .searchable(text: $searchInput, prompt: "Search")
            .navigationDestination(for: File.self, destination: { meta in
                if meta.fileType == .Folder {
                    FileListView(parent: meta, haveScrollView: true)
                        .navigationTitle(meta.name)
                } else {
                    WorkspaceView(DI.workspace, DI.coreService.corePtr)
                        .id(workspace.workspaceViewId)
                        .navigationBarTitleDisplayMode(.inline)
                        .toolbar {
                            ToolbarItemGroup {
                                if workspace.openTabs > 1 {
                                    Button(action: {
                                        DI.sheets.tabsList = true
                                    }, label: {
                                        ZStack {
                                            Label("Tabs", systemImage: "rectangle.fill")
                                            
                                            Text(workspace.openTabs < 100 ? String(workspace.openTabs) : ":D")
                                                .font(.callout)
                                                .foregroundColor(.white)
                                        }
                                    })
                                    .foregroundColor(.blue)
                                }
                                
                                Button(action: {
                                    DI.sheets.sharingFileInfo = meta
                                }, label: {
                                    Label("Share", systemImage: "person.wave.2.fill")
                                })
                                .foregroundColor(.blue)
                                
                                Button(action: {
                                    exportFilesAndShowShareSheet(metas: [meta])
                                }, label: {
                                    Label("Share externally to...", systemImage: "square.and.arrow.up.fill")
                                })
                                .foregroundColor(.blue)
                            }
                        }
                        .onDisappear {
                            DI.workspace.workspaceViewId = UUID()
                        }
                }
            })
            .navigationDestination(isPresented: $settings.showView, destination: {
                SettingsView()
            })
            .navigationDestination(isPresented: $share.showPendingSharesView, destination: {
                PendingSharesView()
            })
            .navigationDestination(isPresented: $billing.showManageSubscriptionView, destination: {
                ManageSubscription()
            })
            .refreshable {
                DI.workspace.requestSync()
            }
    }
}

struct ConstrainedHomeView: View {
    @EnvironmentObject var search: SearchService
    
    @Binding var searchInput: String
    
    @Environment(\.isSearching) var isSearching
    @Environment(\.colorScheme) var colorScheme
    @Environment(\.dismissSearch) private var dismissSearch
    
    @EnvironmentObject var files: FileService
    
    var body: some View {
        ScrollView {
            if search.isPathAndContentSearching {
                if search.isPathAndContentSearchInProgress {
                    ProgressView()
                        .frame(width: 20, height: 20)
                        .padding(.top)
                }
                
                if !search.pathAndContentSearchResults.isEmpty {
                    searchResultsView
                } else if !search.isPathAndContentSearchInProgress && !search.pathAndContentSearchQuery.isEmpty {
                    noSearchResultsView
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
        .navigationTitle(DI.accounts.account?.username ?? "...")
    }
    
    var noSearchResultsView: some View {
        Group {
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
        VStack(alignment: .leading) {
            if files.suggestedDocs?.isEmpty != true {
                Section(header: Text("Suggested")
                    .bold()
                    .foregroundColor(.primary)
                    .textCase(.none)
                    .font(.headline)
                    .padding(.bottom, 3)
                    .padding(.top, 8)) {
                        SuggestedDocs(isiOS: true)
                    }
                    .padding(.horizontal, 20)
            }
                
            Section(header: Text("Files")
                .bold()
                .foregroundColor(.primary)
                .textCase(.none)
                .font(.headline)
                .padding(.bottom, 3)
                .padding(.top, 8)) {
                if let root = files.root {
                    FileListView(parent: root, haveScrollView: false)
                } else {
                    ProgressView()
                        .padding(.leading)
                }
            }
            .padding(.horizontal, 20)
        }
    }
}

struct FileListView: View {
    @EnvironmentObject var files: FileService
    @EnvironmentObject var selected: SelectedFilesState
    @EnvironmentObject var share: ShareService
    @EnvironmentObject var workspace: WorkspaceState

    @Environment(\.colorScheme) var colorScheme
    
    var parent: File
    var haveScrollView: Bool = false
    
    var children: [File] {
        get {
            return files.childrenOf(parent)
        }
    }
    
    var body: some View {
        VStack(spacing: 0) {
            if children.isEmpty {
                emptyView
            } else {
                childrenView
            }
        }
        .padding(.bottom, 100)
        .modifier(FilesListScrollViewModifier(haveScrollView: haveScrollView, isEmptyView: children.isEmpty))
        .toolbar {
            if selected.selectedFiles == nil {
                ToolbarItemGroup {
                    Button(action: {
                        withAnimation(.linear(duration: 0.2)) {
                            selected.selectedFiles = []
                        }
                    }, label: {
                        Text("Edit")
                            .foregroundStyle(.blue)
                    })
                    
                    Button(action: {
                        DI.share.showPendingSharesView = true
                    }, label: {
                        pendingShareToolbarIcon(isPendingSharesEmpty: share.pendingShares?.isEmpty ?? false)
                    })
                    
                    Button(action: {
                        DI.settings.showView = true
                    }, label: {
                        Image(systemName: "gearshape.fill").foregroundColor(.blue)
                    })
                }
            } else {
                ToolbarItem(placement: .topBarLeading) {
                    Button(action: {
                        if selected.selectedFiles?.isEmpty == false {
                            withAnimation(.linear(duration: 0.2)) {
                                selected.selectedFiles = []
                            }
                        } else {
                            for child in files.childrenOfParent() {
                                withAnimation(.linear(duration: 0.2)) {
                                    selected.addFileToSelection(file: child)
                                }
                            }
                        }
                    }, label: {
                        if selected.selectedFiles?.isEmpty == false {
                            Text("Deselect All")
                                .foregroundStyle(.blue)
                        } else {
                            Text("Select All")
                                .foregroundStyle(.blue)
                        }
                        
                    })
                    .navigationBarBackButtonHidden()
                }
                
                ToolbarItem(placement: .topBarTrailing) {
                    Button(action: {
                        withAnimation(.linear(duration: 0.2)) {
                            selected.selectedFiles = nil
                        }
                    }, label: {
                        Text("Done")
                            .foregroundStyle(.blue)
                    })
                }
            }
        }
    }
    
    var childrenView: some View {
        ForEach(files.childrenOf(parent), id: \.self) { meta in
            FileCell(meta: meta, selectedFiles: selected.selectedFiles)
        }
    }
    
    var emptyView: some View {
        VStack {
            Spacer()
            
            Image(systemName: "doc")
                .font(.system(size: 130))
                .padding(15)
            
            Text("This folder is empty.")
                .font(.callout)
            
            Spacer()
        }
    }
}

struct FilesListScrollViewModifier: ViewModifier {
    var haveScrollView: Bool
    var isEmptyView: Bool
    
    func body(content: Content) -> some View {
        Group {
            if haveScrollView {
                if isEmptyView {
                    GeometryReader { geometry in
                        ScrollView {
                            content
                                .frame(width: geometry.size.width)
                                .frame(minHeight: geometry.size.height)
                        }
                        .refreshable {
                            DI.workspace.requestSync()
                        }
                    }
                } else {
                    ScrollView {
                        content
                    }
                    .refreshable {
                        DI.workspace.requestSync()
                    }
                }
            } else {
                content
            }
        }
    }
}
