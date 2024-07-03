import SwiftUI
import SwiftWorkspace
import SwiftLockbookCore
import Foundation

struct ConstrainedHomeViewWrapper: View {
    
    @EnvironmentObject var workspace: WorkspaceState
    @EnvironmentObject var files: FileService
    @EnvironmentObject var search: SearchService
    
    @State var searchInput: String = ""
    
    var body: some View {
        ZStack {
            VStack {
                NavigationStack(path: $files.path) {
                    VStack {
                        ConstrainedHomeView(searchInput: $searchInput)
                            .searchable(text: $searchInput, prompt: "Search...")
                            .navigationDestination(for: File.self, destination: { meta in
                                if meta.fileType == .Folder {
                                    FileListView(parent: meta, haveScrollView: true)
                                        .navigationTitle(meta.name)
                                } else {
                                    WorkspaceView(DI.workspace, DI.coreService.corePtr)
                                        .equatable()
                                        .navigationBarTitleDisplayMode(.inline)
                                        .toolbar {
                                            ToolbarItemGroup {
                                                Button(action: {
                                                    DI.sheets.sharingFileInfo = meta
                                                }, label: {
                                                    Label("Share", systemImage: "person.wave.2.fill")
                                                })
                                                .foregroundColor(.blue)
                                                .padding(.trailing, 10)
                                                
                                                Button(action: {
                                                    exportFileAndShowShareSheet(meta: meta)
                                                }, label: {
                                                    Label("Share externally to...", systemImage: "square.and.arrow.up.fill")
                                                })
                                                .foregroundColor(.blue)
                                                .padding(.trailing, 10)
                                            }
                                        }
                                        .background(.red)
                                }
                                
                            })
                    }
                }
            }
            .onChange(of: files.path) { new in
                if files.path.last?.fileType != .Document && DI.workspace.openDoc != nil {
                    DI.workspace.closeActiveTab = true
                }
            }
            
            if files.path.last?.fileType != .Document {
                WorkspaceView(DI.workspace, DI.coreService.corePtr)
                    .equatable()
                    .opacity(0)
            }
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
                suggestAndFilesView
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
        .navigationBarTitle(DI.accounts.account!.username)
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
        VStack(spacing: 0) {
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
    }
    
    var suggestAndFilesView: some View {
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
                
                if let root = files.root {
                    Section(header: Text("Files")
                        .bold()
                        .foregroundColor(.primary)
                        .textCase(.none)
                        .font(.headline)
                        .padding(.bottom, 3)
                        .padding(.top, 8)) {
                            FileListView(parent: root, haveScrollView: false)
                        }
                        .padding(.horizontal, 20)
                } else {
                    ProgressView()
                }
                
            }
        }
    }
}

struct FileListView: View {
    @EnvironmentObject var files: FileService
    @EnvironmentObject var share: ShareService
    @EnvironmentObject var onboarding: OnboardingService

    @Environment(\.colorScheme) var colorScheme
    
    var parent: File
    var haveScrollView: Bool = false
    
    var children: [File] {
        get {
            return files.childrenOf(parent)
        }
    }
    
    var body: some View {
        VStack {
            if children.isEmpty {
                emptyView
            } else {
                if haveScrollView {
                    ScrollView {
                        childrenView
                    }
                } else {
                    childrenView
                }
            }
        }
        .toolbar {
            ToolbarItemGroup {
                NavigationLink(
                    destination: PendingSharesView()) {
                        pendingShareToolbarIcon(isPendingSharesEmpty: share.pendingShares?.isEmpty ?? false)
                    }
                
                NavigationLink(
                    destination: SettingsView().equatable(), isActive: $onboarding.theyChoseToBackup) {
                        Image(systemName: "gearshape.fill").foregroundColor(.blue)
                            .padding(.horizontal, 10)
                    }
            }
        }
    }
    
    var childrenView: some View {
        ForEach(files.childrenOf(parent)) { meta in
            FileCell(meta: meta)
                .padding(.horizontal)
        }
        .listRowBackground(Color.clear)
        .listRowInsets(EdgeInsets())
        .listRowSeparator(.hidden)
    }
    
    var emptyView: some View {
        VStack {
            Spacer()
            
            Image(systemName: "questionmark.folder")
                .font(.system(size: 130))
                .padding(15)
            
            Text("This folder is empty.")
                .font(.callout)
            
            Spacer()
        }
    }
}

extension UIScreen {
    static var current: UIScreen? {
        for scene in UIApplication.shared.connectedScenes {
            guard let windowScene = scene as? UIWindowScene else { continue }
            for window in windowScene.windows {
                if window.isKeyWindow { return window.screen }
            }
        }
        return nil
    }
}
