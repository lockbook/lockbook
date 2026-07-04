import SwiftUI
import SwiftWorkspace

struct HomeView: View {
    @Environment(\.isPreview) private var isPreview
    #if os(iOS)
        @Environment(\.horizontalSizeClass) private var horizontalSizeClass
    #endif

    @State private var homeState = HomeState()
    @AppStorage("sidebarTab") private var selectedTab: SidebarTab = .files
    @State private var showCreateFile = false
    @State private var quickCreateCount = 0
    @State private var shareTarget: File? = nil
    @State private var showTabsSidebar = false
    #if os(iOS)
        @State private var showSettings = false
        @State private var keyboardVisible = false
        @State private var showCreateAlongside = false
    #endif

    @State private var filesModel: FilesModel
    @State private var fileTreeModel: FileTreeModel
    @State private var sharedTreeModel: FileTreeModel
    @State private var recentsModel = RecentsModel()
    #if os(iOS)
        @State private var searchModel: SearchModel
    #endif

    @State private var workspaceInput = WorkspaceInputState(coreHandle: AppState.lb.lbUnsafeRawPtr)
    @State private var workspaceOutput = WorkspaceOutputState()

    init() {
        let filesModel = FilesModel()

        _filesModel = State(initialValue: filesModel)
        _fileTreeModel = State(initialValue: FileTreeModel(filesModel: filesModel))
        _sharedTreeModel = State(initialValue: FileTreeModel(filesModel: filesModel))
        #if os(iOS)
            _searchModel = State(initialValue: SearchModel(filesModel: filesModel))
        #endif
    }

    var body: some View {
        @Bindable var homeState = homeState

        NavigationSplitView(
            columnVisibility: $homeState.splitViewVisibility,
            preferredCompactColumn: $homeState.compactColumn
        ) {
            sidebar
                .navigationSplitViewColumnWidth(min: 250, ideal: 300)
        } detail: {
            workspace
                .navigationTitle(openDocFile?.name ?? "Lockbook")
                #if os(iOS)
                    .navigationBarTitleDisplayMode(.inline)
                #endif
                .toolbar {
                    #if os(iOS)
                        if horizontalSizeClass == .regular,
                           homeState.splitViewVisibility != .all
                        {
                            ToolbarItem(placement: .topBarLeading) {
                                Button {
                                    withAnimation {
                                        homeState.splitViewVisibility = .all
                                    }
                                } label: {
                                    Image(systemName: "sidebar.left")
                                }
                            }
                        }

                    #endif

                    if let file = openDocFile {
                        ToolbarItem(placement: sharePlacement) {
                            Button {
                                shareTarget = file
                            } label: {
                                Image(systemName: "square.and.arrow.up")
                            }
                        }
                    }

                    if workspaceOutput.tabCount > 0 {
                        ToolbarItem(placement: sharePlacement) {
                            Button {
                                showTabsSidebar.toggle()
                            } label: {
                                ZStack(alignment: .center) {
                                    RoundedRectangle(cornerSize: .init(width: 4, height: 4))
                                        .stroke(lineWidth: 2)
                                        .frame(width: 18, height: 18)

                                    Text(workspaceOutput.tabCount < 100 ? String(workspaceOutput.tabCount) : ":D")
                                        .font(.footnote)
                                }
                            }
                        }
                    }
                }
                .inspector(isPresented: inspectorPresented) {
                    WorkspaceTabsList()
                        .inspectorColumnWidth(min: 220, ideal: 260, max: 320)
                }
                .sheet(item: $shareTarget) { file in
                    ShareFileSheet(id: file.id)
                }
                #if os(iOS)
                    .sheet(isPresented: $showCreateAlongside) {
                        CreateFileSheet(fileTreeModel: fileTreeModel, alongside: openDocFile != nil)
                    }
                    .overlay(alignment: .bottom) {
                        if !keyboardVisible {
                            HStack {
                                detailSearchButton

                                Spacer()

                                detailCreateButton
                            }
                            .padding(.horizontal, 20)
                            .transition(.opacity)
                        }
                    }
                    .animation(.easeInOut(duration: 0.2), value: keyboardVisible)
                #endif
        }
        .overlay {
            if homeState.openingLink {
                StatusHUD(message: "Opening link")
                    .transition(.opacity.combined(with: .scale(scale: 0.94)))
            } else if filesModel.importsInProgress > 0 {
                StatusHUD(message: "Importing files")
                    .transition(.opacity.combined(with: .scale(scale: 0.94)))
            } else if filesModel.exportsInProgress > 0 {
                StatusHUD(message: "Exporting files")
                    .transition(.opacity.combined(with: .scale(scale: 0.94)))
            } else if let result = filesModel.importResult {
                ImportResultHUD(result: result)
                    .transition(.opacity.combined(with: .scale(scale: 0.94)))
            } else if let result = filesModel.moveResult {
                MoveResultHUD(result: result)
                    .transition(.opacity.combined(with: .scale(scale: 0.94)))
            }
        }
        .animation(.easeInOut(duration: 0.2), value: filesModel.moveResult)
        .animation(.easeInOut(duration: 0.2), value: homeState.openingLink)
        .animation(.easeInOut(duration: 0.2), value: filesModel.importsInProgress)
        .animation(.easeInOut(duration: 0.2), value: filesModel.exportsInProgress)
        .animation(.easeInOut(duration: 0.2), value: filesModel.importResult)
        #if os(iOS)
            .overlay {
                tabsDrawer
            }
        #endif
        .modifier(OpenLbLinkModifier())
        .environment(homeState)
        .environment(filesModel)
        .environment(workspaceInput)
        .environment(workspaceOutput)
        .onChange(of: workspaceOutput.openDoc) { _, openDoc in
            guard let openDoc else { return }
            fileTreeModel.docOpened(openDoc)
            sharedTreeModel.docOpened(openDoc)
        }
        .onChange(of: workspaceOutput.selectedFolder) { _, selectedFolder in
            guard let selectedFolder else { return }
            fileTreeModel.folderSelected(selectedFolder)
            sharedTreeModel.folderSelected(selectedFolder)
        }
        .onChange(of: AppState.lb.events.metadataVersion) {
            filesModel.loadFiles()
        }
        .onChange(of: AppState.lb.events.status) { _, status in
            filesModel.recomputeStatusDots(status: status)
        }
        .onChange(of: workspaceOutput.tabCount) { _, count in
            if count == 0 {
                showTabsSidebar = false
            }
        }
        #if os(iOS)
            .onReceive(NotificationCenter.default.publisher(for: UIResponder.keyboardWillShowNotification)) { _ in
                keyboardVisible = true
            }
            .onReceive(NotificationCenter.default.publisher(for: UIResponder.keyboardWillHideNotification)) { _ in
                keyboardVisible = false
            }
        #endif
    }

    private var inspectorPresented: Binding<Bool> {
        Binding(
            get: {
                #if os(iOS)
                    showTabsSidebar && horizontalSizeClass == .regular
                #else
                    showTabsSidebar
                #endif
            },
            set: { showTabsSidebar = $0 }
        )
    }

    #if os(iOS)
        @ViewBuilder
        private var detailSearchButton: some View {
            Group {
                if workspaceOutput.openDoc != nil {
                    Menu {
                        Button {
                            workspaceInput.showFindInDoc()
                        } label: {
                            Label("Search this document", systemImage: "doc.text.magnifyingglass")
                        }

                        Button {
                            searchEverywhere()
                        } label: {
                            Label("Search everywhere", systemImage: "magnifyingglass")
                        }
                    } label: {
                        Image(systemName: "magnifyingglass")
                            .frame(width: 26, height: 26)
                    }
                } else {
                    Button {
                        searchEverywhere()
                    } label: {
                        Image(systemName: "magnifyingglass")
                            .frame(width: 26, height: 26)
                    }
                }
            }
            .buttonStyle(.glass)
            .buttonBorderShape(.circle)
            .tint(.primary)
        }

        private var detailCreateButton: some View {
            Image(systemName: "square.and.pencil")
                .frame(width: 26, height: 26)
                .padding(9)
                .glassEffect(.regular.interactive(), in: Circle())
                .contentShape(Circle())
                .onTapGesture { showCreateAlongside = true }
                .onLongPressGesture { quickCreateAlongside() }
                .sensoryFeedback(.impact(weight: .medium), trigger: quickCreateCount)
        }

        private func quickCreateAlongside() {
            let parent = openDocFile.flatMap { filesModel.idsToFiles[$0.parent] } ?? filesModel.root
            guard let parent else { return }

            quickCreateCount += 1
            workspaceInput.createDocAt(parent: parent.id, drawing: false)
        }

        private func searchEverywhere() {
            selectedTab = .search
            withAnimation {
                homeState.compactColumn = .sidebar
                homeState.splitViewVisibility = .all
            }
        }

        private var tabsDrawer: some View {
            ZStack(alignment: .trailing) {
                if showTabsSidebar, horizontalSizeClass == .compact {
                    Color.black.opacity(0.35)
                        .ignoresSafeArea()
                        .onTapGesture {
                            showTabsSidebar = false
                        }
                        .transition(.opacity)

                    WorkspaceTabsList()
                        .scrollContentBackground(.hidden)
                        .frame(width: 310)
                        .background(.regularMaterial, ignoresSafeAreaEdges: .all)
                        .transition(.move(edge: .trailing))
                        .gesture(
                            DragGesture().onEnded { value in
                                if value.translation.width > 60 {
                                    showTabsSidebar = false
                                }
                            }
                        )
                }
            }
            .animation(.easeInOut(duration: 0.22), value: showTabsSidebar)
        }
    #endif

    private var sidebar: some View {
        VStack(spacing: 0) {
            #if os(macOS)
                sidebarActions
            #endif

            sidebarContent
        }
    }

    private var sidebarContent: some View {
        Group {
            switch selectedTab {
            case .files:
                FileTreeView(fileTreeModel: fileTreeModel)
            case .recents:
                RecentsView(model: recentsModel, fileTreeModel: fileTreeModel)
            case .sharedWithMe:
                SharedWithMeView(fileTreeModel: sharedTreeModel)
            #if os(iOS)
                case .search:
                    SearchTabView(model: searchModel)
            #endif
            }
        }
        .toolbar {
            ToolbarItem(placement: tabstripPlacement) {
                Picker("Tabs", selection: $selectedTab) {
                    ForEach([SidebarTab.files, .recents, .sharedWithMe]) { tab in
                        Label(tab.title, systemImage: tab.systemImage)
                            .tag(tab)
                    }
                }
                .pickerStyle(.segmented)
                .fixedSize()
            }

            #if os(iOS)
                ToolbarItem(placement: .topBarTrailing) {
                    Button {
                        showSettings = true
                    } label: {
                        Image(systemName: "gearshape")
                    }
                }

                ToolbarItem(placement: .topBarTrailing) {
                    Button {
                        homeState.splitViewVisibility = .detailOnly
                        homeState.compactColumn = .detail
                    } label: {
                        Image(systemName: "sidebar.left")
                            .imageScale(.large)
                    }
                }

                if horizontalSizeClass == .compact {
                    ToolbarItem(placement: .bottomBar) {
                        Button {
                            selectedTab = .search
                        } label: {
                            Image(systemName: "magnifyingglass")
                        }
                    }

                    ToolbarSpacer(.flexible, placement: .bottomBar)

                    ToolbarItem(placement: .bottomBar) {
                        Image(systemName: "square.and.pencil")
                            .onTapGesture { showCreateFile = true }
                            .onLongPressGesture { quickCreateDoc() }
                            .sensoryFeedback(.impact(weight: .medium), trigger: quickCreateCount)
                    }
                }
            #endif
        }
        .sheet(isPresented: $showCreateFile) {
            CreateFileSheet(fileTreeModel: fileTreeModel)
        }
        #if os(iOS)
            .sheet(isPresented: $showSettings) {
                NavigationStack {
                    SettingsView()
                }
            }
        #endif
    }

    #if os(macOS)
        private var sidebarActions: some View {
            HStack(spacing: 8) {
                actionChip("New", systemImage: "square.and.pencil")
                    .onTapGesture { showCreateFile = true }
                    .onLongPressGesture { quickCreateDoc() }
                    .sensoryFeedback(.impact(weight: .medium), trigger: quickCreateCount)

                actionChip("Search", systemImage: "magnifyingglass")
                    .onTapGesture { workspaceInput.showSearch() }
            }
            .padding(.horizontal, 10)
            .padding(.vertical, 8)
        }

        private func actionChip(_ title: String, systemImage: String) -> some View {
            Label(title, systemImage: systemImage)
                .font(.callout)
                .frame(maxWidth: .infinity)
                .padding(.vertical, 6)
                .background(.quaternary.opacity(0.6), in: RoundedRectangle(cornerRadius: 7))
                .contentShape(RoundedRectangle(cornerRadius: 7))
        }
    #endif

    private func quickCreateDoc() {
        guard let root = filesModel.root else { return }

        quickCreateCount += 1
        workspaceInput.createDocAt(parent: root.id, drawing: false)
        homeState.compactColumn = .detail
    }

    private var tabstripPlacement: ToolbarItemPlacement {
        #if os(macOS)
            .principal
        #else
            .topBarLeading
        #endif
    }

    private var sharePlacement: ToolbarItemPlacement {
        #if os(macOS)
            .primaryAction
        #else
            .topBarTrailing
        #endif
    }

    private var openDocFile: File? {
        guard let id = workspaceOutput.openDoc else {
            return nil
        }

        return filesModel.idsToFiles[id]
    }

    @ViewBuilder
    private var workspace: some View {
        if isPreview {
            Text("Workspace")
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        } else {
            #if os(iOS)
                WorkspaceView()
            #else
                WorkspaceView(workspaceInput, workspaceOutput, AppState.lb.lbUnsafeRawPtr)
            #endif
        }
    }
}

enum SidebarTab: String, CaseIterable, Identifiable {
    case files
    case recents
    case sharedWithMe
    #if os(iOS)
        case search
    #endif

    var id: Self {
        self
    }

    var title: String {
        switch self {
        case .files: "Files"
        case .recents: "Recents"
        case .sharedWithMe: "Shared"
        #if os(iOS)
            case .search: "Search"
        #endif
        }
    }

    var systemImage: String {
        switch self {
        case .files: "folder.fill"
        case .recents: "clock.fill"
        case .sharedWithMe: "person.2.fill"
        #if os(iOS)
            case .search: "magnifyingglass"
        #endif
        }
    }
}

#Preview {
    HomeView()
}
