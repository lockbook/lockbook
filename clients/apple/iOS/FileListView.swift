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
                ConstrainedHomeView(searchInput: $searchInput)
            }
            .searchable(text: $searchInput, prompt: "Search...")
            
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
                    .padding(.bottom, 3)) {
                        SuggestedDocs(isiOS: true)
                    }
                    .padding(.horizontal, 20)
            }
            
            Section(header: Text("Files")
                .bold()
                .foregroundColor(.primary)
                .textCase(.none)
                .font(.headline)
                .padding(.bottom, 3)) {
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

struct FileListView: View {
    @EnvironmentObject var sheets: SheetState
    @EnvironmentObject var fileService: FileService
    @EnvironmentObject var search: SearchService
    @EnvironmentObject var sync: SyncService
    @EnvironmentObject var workspace: WorkspaceState
    
    @Environment(\.colorScheme) var colorScheme

    @State var searchInput: String = ""
    @State var navigateToManageSub: Bool = false
    @State private var mainViewOffset = CGSize.zero
    @State private var mainViewOpacity: Double = 1
    
    @State private var hideOutOfSpaceAlert = UserDefaults.standard.bool(forKey: "hideOutOfSpaceAlert")
    
    var body: some View {
        ZStack {
            SearchWrapperView(
                searchInput: $searchInput,
                mainView: mainView,
                isiOS: true)
            .searchable(text: $searchInput, prompt: "Search")
        }
        .gesture(
            DragGesture().onEnded({ (value) in
                if value.translation.width > 50 && fileService.parent?.isRoot == false {
                    fileService.upADirectory()
                }
            }))
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
    
    @ViewBuilder
    var mainView: some View {
        Group {
            List {
                if fileService.parent?.isRoot == true && fileService.suggestedDocs?.isEmpty != true {
                    Section(header: Text("Suggested")
                        .bold()
                        .foregroundColor(.primary)
                        .textCase(.none)
                        .font(.headline)
                        .padding(.bottom, 3)) {
                            SuggestedDocs(isiOS: true)
                        }
                        .offset(mainViewOffset)
                        .opacity(mainViewOpacity)
                }
                
                Section(header: Text("Files")
                    .bold()
                    .foregroundColor(.primary)
                    .textCase(.none)
                    .font(.headline)
                    .padding(.bottom, 3)) {
                        EmptyView()
                    }
                    .offset(mainViewOffset)
                    .opacity(mainViewOpacity)
            }
            .navigationBarTitle(fileService.parent.map{($0.name)} ?? "")
            .modifier(DragGestureViewModifier(onUpdate: { gesture in
                if fileService.parent?.isRoot == false && gesture.translation.width < 200 && gesture.translation.width > 0 {
                    mainViewOffset.width = gesture.translation.width
                }
            }, onEnd: { gesture in
                if gesture.translation.width > 100 && fileService.parent?.isRoot == false {
                    animateToParentFolder() {
                        fileService.upADirectory()
                    }
                } else {
                    withAnimation {
                        mainViewOffset.width = 0
                    }
                }
            }))
            
            FilePathBreadcrumb() { file in
                animateToParentFolder() {
                    fileService.pathBreadcrumbClicked(file)
                }
            }
            
            BottomBar(isiOS: true)
        }
    }

    var files: some View {
        let children = fileService.childrenOfParent()

        return ForEach(children) { meta in
            EmptyView()
        }
        .listRowBackground(Color.clear)
        .listRowInsets(EdgeInsets())
        .listRowSeparator(.hidden)
    }

    func animateToParentFolder(realParentUpdate: @escaping () -> Void) {
        withAnimation(.linear(duration: 0.2)) {
            mainViewOffset.width = 200
            mainViewOpacity = 0
        }

        DispatchQueue.main.asyncAfter(deadline: .now() + 0.2) {
            mainViewOffset.width = -200
            mainViewOpacity = 1

            realParentUpdate()

            DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) {
                withAnimation(.linear(duration: 0.1)) {
                    mainViewOffset.width = 0
                }
            }
        }
    }
}

struct DragGestureViewModifier: ViewModifier {
    @GestureState private var isDragging: Bool = false
    @State private var gestureState: GestureStatus = .idle

    var onUpdate: (DragGesture.Value) -> Void
    var onEnd: (DragGesture.Value) -> Void

    func body(content: Content) -> some View {
        content
            .gesture(
                DragGesture()
                    .updating($isDragging) { _, isDragging, _ in
                        isDragging = true
                    }
                    .onChanged(onDragChange(_:))
                    .onEnded(onDragEnded(_:))
            )
            .onChange(of: gestureState) { state in
                guard state == .started else { return }
                gestureState = .active
            }
            .onChange(of: isDragging) { value in
                if value, gestureState != .started {
                    gestureState = .started
                } else if !value, gestureState != .ended {
                    gestureState = .cancelled
                }
            }
    }

    func onDragChange(_ value: DragGesture.Value) {
        guard gestureState == .started || gestureState == .active else { return }
        onUpdate(value)
    }

    func onDragEnded(_ value: DragGesture.Value) {
        gestureState = .ended
        onEnd(value)
    }

    enum GestureStatus: Equatable {
        case idle
        case started
        case active
        case ended
        case cancelled
    }
}

struct FileListView_Previews: PreviewProvider {
    static var previews: some View {
        NavigationView {
            FileListView()
                .mockDI()
        }
    }
}
