import SwiftUI
import SwiftWorkspace
import SwiftLockbookCore
import Foundation

struct ConstrainedHomeViewWrapper: View {
    
    @EnvironmentObject var workspace: WorkspaceState
    
    @State var searchInput: String = ""
    
    var body: some View {
        ZStack {
            NavigationView {
                VStack {
                    ConstrainedHomeView(searchInput: $searchInput)
                        .searchable(text: $searchInput, prompt: "Search...")
                    
                    FilePathBreadcrumb()
                    
                    BottomBar(isiOS: true)
                }
            }
            
            NavigationView {
                WorkspaceView(DI.workspace, DI.coreService.corePtr)
                    .equatable()
                    .toolbar {
                        ToolbarItem(placement: .navigationBarLeading) {
                            Button(action: {
                                workspace.closeActiveTab = true
                            }) {
                                HStack {
                                    Image(systemName: "chevron.backward")
                                        .foregroundStyle(.blue)
                                        .bold()
                                    
                                    Text(DI.accounts.account!.username)
                                        .foregroundStyle(.blue)
                                }
                            }
                        }
                        
                        ToolbarItemGroup {
                            if let id = workspace.openDoc {
                                if let meta = DI.files.idsAndFiles[id] {
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
                        }
                    }
            }
            .offset(x: workspace.currentTab != .Welcome ? workspace.dragOffset : UIScreen.current?.bounds.width ?? 0)
        }
    }
}

struct ConstrainedHomeView: View {
    @EnvironmentObject var files: FileService
    @EnvironmentObject var search: SearchService
    
    @Binding var searchInput: String
    
    @Environment(\.isSearching) var isSearching
    @Environment(\.colorScheme) var colorScheme
    @Environment(\.dismissSearch) private var dismissSearch
    
    var body: some View {
        ScrollView {
            Divider()
                .frame(height: 10)
                .foregroundStyle(.black)
                .padding(0)

            if search.isPathAndContentSearching {
                if search.isPathAndContentSearchInProgress {
                    ProgressView()
                        .frame(width: 20, height: 20)
                        .padding(.top)
                }
                
                if !search.pathAndContentSearchResults.isEmpty {
                    VStack(spacing: 0) {
                        ForEach(search.pathAndContentSearchResults) { result in
                            switch result {
                            case .PathMatch(_, let meta, let name, let path, let matchedIndices, _):
                                Button(action: {
                                    DI.workspace.requestOpenDoc(meta.id)
                                    dismissSearch()
                                }) {
                                    SearchFilePathCell(name: name, path: path, matchedIndices: matchedIndices)
                                }
                            case .ContentMatch(_, let meta, let name, let path, let paragraph, let matchedIndices, _):
                                Button(action: {
                                    DI.workspace.requestOpenDoc(meta.id)
                                    dismissSearch()
                                }) {
                                    SearchFileContentCell(name: name, path: path, paragraph: paragraph, matchedIndices: matchedIndices)
                                }
                            }
                        }
                    }
                    .padding(.horizontal)
                    .background(RoundedRectangle(cornerRadius: 10).fill(colorScheme == .light ? .white : Color(uiColor: .secondarySystemBackground)))
                } else if !search.isPathAndContentSearchInProgress && !search.pathAndContentSearchQuery.isEmpty {
                    Text("No results.")
                        .font(.headline)
                        .foregroundColor(.gray)
                        .fontWeight(.bold)
                        .padding()
                    
                    Spacer()
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
        .navigationBarTitle(files.parent.map{$0.name} ?? "")
    }
    
    var suggestAndFilesView: some View {
        VStack(alignment: .leading) {
            if files.parent?.isRoot == true && files.suggestedDocs?.isEmpty != true {
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
                    VStack {
                        ForEach(files.childrenOfParent()) { meta in
                            FileCell(meta: meta)
                                .padding(.horizontal)
                        }
                        .listRowBackground(Color.clear)
                        .listRowInsets(EdgeInsets())
                        .listRowSeparator(.hidden)
                    }
                    .background(RoundedRectangle(cornerRadius: 10).fill(colorScheme == .light ? .white : Color(uiColor: .secondarySystemBackground)))
                }
                .padding(.horizontal, 20)
        }
    }
}

// NOT DETECTING TOUCH SINCE IT IS A SCROLLVIEW!!!!!!!!!
struct SlidingListView: UIViewControllerRepresentable {
    var items: [String]
    
    func makeUIViewController(context: Context) -> SlidingListViewController {
        let viewController = SlidingListViewController()
        viewController.items = items
        return viewController
    }
    
    func updateUIViewController(_ uiViewController: SlidingListViewController, context: Context) {
        // Optionally handle updates
    }
}

class SlidingListViewController: UIViewController, UITableViewDataSource, UITableViewDelegate {
    var currentList: UITableView!
    private var nextList: UITableView?
    
    var items: [String] = []
    var nextItems: [String] = []
    
    override func viewDidLoad() {
        super.viewDidLoad()
        
        setupCurrentList()
    }
    
    private func setupCurrentList() {
        print("Setting up current list")
        currentList = UITableView(frame: view.bounds, style: .plain)
        currentList.dataSource = self
        currentList.delegate = self
        currentList.register(UITableViewCell.self, forCellReuseIdentifier: "cell")
        currentList.isUserInteractionEnabled = true
        currentList.allowsSelection = true
        currentList.reloadData()
        view.addSubview(currentList)
    }
    
    func setNextItems(_ items: [String]) {
        print("Setting next items!")
        nextItems = items
        
        nextList = UITableView(frame: view.bounds.offsetBy(dx: view.bounds.width, dy: 0), style: .plain)
        nextList?.dataSource = self
        nextList?.delegate = self
        nextList?.register(UITableViewCell.self, forCellReuseIdentifier: "cell")
        nextList?.isUserInteractionEnabled = true
        view.addSubview(nextList!)
        
        UIView.animate(withDuration: 0.3, animations: {
            self.currentList.frame = self.view.bounds.offsetBy(dx: -self.view.bounds.width, dy: 0)
            self.nextList?.frame = self.view.bounds
        }) { _ in
            print("Animation completed")
            self.currentList.removeFromSuperview()
            self.currentList = self.nextList
            self.nextList = nil
        }
    }
    
    func tableView(_ tableView: UITableView, numberOfRowsInSection section: Int) -> Int {
        print("Number of rows in section: \(section)")
        if tableView == currentList {
            return items.count
        } else {
            return nextItems.count
        }
    }
    
    func tableView(_ tableView: UITableView, cellForRowAt indexPath: IndexPath) -> UITableViewCell {
        print("Making cell for row at \(indexPath.row)")
        let cell = tableView.dequeueReusableCell(withIdentifier: "cell", for: indexPath)
        if tableView == currentList {
            cell.textLabel?.text = items[indexPath.row]
        } else {
            cell.textLabel?.text = nextItems[indexPath.row]
        }
        return cell
    }
    
    func tableView(_ tableView: UITableView, didSelectRowAt indexPath: IndexPath) {
        print("Selecting row at \(indexPath.row)")
        let newItems = ["Item 1", "Item 2", "Item 3"]
        setNextItems(newItems)
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
