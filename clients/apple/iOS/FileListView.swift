import SwiftUI
import SwiftWorkspace
import SwiftLockbookCore
import Foundation

class SwiftUITableViewCell: UITableViewCell {
    private var hostingController: UIHostingController<FileCell>?

    func configure(with meta: File, enterFolderAnim: @escaping (File) -> Void) {
        // If the hosting controller already exists, update it
        if let hostingController = hostingController {
            hostingController.rootView = FileCell(meta: meta, enterFolderAnim: enterFolderAnim)
            hostingController.view.invalidateIntrinsicContentSize()
            return
        }
        
        // Otherwise, create a new hosting controller
        let fileCell = FileCell(meta: meta, enterFolderAnim: enterFolderAnim)
        let hostingController = UIHostingController(rootView: fileCell)
        
        // Add the hosting controller's view to the cell's content view
        contentView.addSubview(hostingController.view)
        
        // Set up constraints
        hostingController.view.translatesAutoresizingMaskIntoConstraints = false
        NSLayoutConstraint.activate([
            hostingController.view.leadingAnchor.constraint(equalTo: contentView.leadingAnchor),
            hostingController.view.trailingAnchor.constraint(equalTo: contentView.trailingAnchor),
            hostingController.view.topAnchor.constraint(equalTo: contentView.topAnchor),
            hostingController.view.bottomAnchor.constraint(equalTo: contentView.bottomAnchor)
        ])
        
        // Save the hosting controller
        self.hostingController = hostingController
    }
}

class TempWrapperView: UIView {
    override init(frame: CGRect) {
        super.init(frame: frame)
    }
    
    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }
}

class FileListViewController: UIViewController, UITableViewDataSource, UITableViewDelegate {
    private let tableView = UITableView()
    private var items: [File] = []
    
    var currentParent: File? = nil
    
    override func viewDidLoad() {
        super.viewDidLoad()
        
        view.addSubview(tableView)
        tableView.translatesAutoresizingMaskIntoConstraints = false
        NSLayoutConstraint.activate([
            tableView.topAnchor.constraint(equalTo: view.topAnchor),
            tableView.bottomAnchor.constraint(equalTo: view.bottomAnchor),
            tableView.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            tableView.trailingAnchor.constraint(equalTo: view.trailingAnchor)
        ])
        
        tableView.dataSource = self
        tableView.delegate = self
        tableView.register(UITableViewCell.self, forCellReuseIdentifier: "SwiftUICell")
    }
    
    func updateItems(_ newItems: [File]) {
        self.items = newItems
        tableView.reloadData()
    }
        
    func tableView(_ tableView: UITableView, numberOfRowsInSection section: Int) -> Int {
        return items.count
    }
    
    func tableView(_ tableView: UITableView, cellForRowAt indexPath: IndexPath) -> UITableViewCell {
//        guard let cell = tableView.dequeueReusableCell(withIdentifier: "SwiftUICell", for: indexPath) as? SwiftUITableViewCell else {
//            return UITableViewCell()
//        }
//        let file = items[indexPath.row]
//        cell.configure(with: file) { meta in
//            
//            
//        }
        
        let cell = UITableViewCell()
        cell.backgroundColor = .red
        return cell
    }
        
    func tableView(_ tableView: UITableView, didSelectRowAt indexPath: IndexPath) {
        let file = items[indexPath.row]
        print("Selected item: \(file.name)")
        
        UIView.transition(
            with: self.view,
            duration: 1,
            animations: {
                var newFrame = tableView.frame
                newFrame.origin.x = self.view.bounds.width
                tableView.frame = newFrame
            },
            completion: nil)
    }
}

struct FileListViewRepresentable: UIViewControllerRepresentable {
    @EnvironmentObject var files: FileService
        
    func makeUIViewController(context: Context) -> FileListViewController {
        let viewController = FileListViewController()
        return viewController
    }
    
    func updateUIViewController(_ uiViewController: FileListViewController, context: Context) {
        if(uiViewController.currentParent != files.parent) {
            uiViewController.currentParent = files.parent
            uiViewController.updateItems(files.childrenOf(uiViewController.currentParent))
        }
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
                        FileListViewRepresentable()
                            .frame(height: 200)
                            .listRowBackground(Color.clear)
                            .listRowInsets(EdgeInsets())
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
                FileCell(meta: meta) { meta in
                    withAnimation(.linear(duration: 0.2)) {
                        mainViewOffset.width = -200
                    }

                    DispatchQueue.main.asyncAfter(deadline: .now() + 0.2) {
                        mainViewOpacity = 0
                        mainViewOffset.width = 200

                        fileService.intoChildDirectory(meta)

                        DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) {
                            withAnimation(.linear(duration: 0.1)) {
                                mainViewOffset.width = 0
                                mainViewOpacity = 1
                            }
                        }
                    }
                }
                .padding(.horizontal)
                .padding(.vertical, 5)
                .background(colorScheme == .light ? .white : Color(uiColor: .secondarySystemBackground))

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
