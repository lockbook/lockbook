import SwiftUI
import SwiftWorkspace

struct HomeView: View {
    @Environment(\.isPreview) private var isPreview
    #if os(iOS)
        @Environment(\.horizontalSizeClass) private var horizontalSizeClass
    #endif

    @State private var homeState = HomeState()
    @AppStorage("sidebarTab") private var selectedTab: SidebarTab = .files
    @AppStorage("hideOutOfSpaceAlert") private var hideOutOfSpaceAlert = false
    @State private var showCreateFile = false
    @State private var quickCreateCount = 0
    @State private var shareTarget: File? = nil
    @State private var renameTarget: File? = nil
    @State private var showTabsSidebar = false
    @State private var showOutOfSpaceAlert = false
    #if os(iOS)
        @State private var showSettings = false
        @State private var keyboardVisible = false
        @State private var showCreateAlongside = false
    #else
        @State private var showImporter = false
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
                .navigationTitle(detailTitle)
                #if os(iOS)
                    .navigationBarTitleDisplayMode(.inline)
                #endif
                .toolbar {
                    if let file = openDocFile {
                        ToolbarItem(placement: .principal) {
                            Button {
                                renameTarget = file
                            } label: {
                                let segments = filesModel.ancestors(of: file).map(\.name)

                                ViewThatFits(in: .horizontal) {
                                    titleCrumb(file, leading: segments)

                                    if segments.count > 1, let parent = segments.last {
                                        titleCrumb(file, leading: ["…", parent])
                                    }

                                    if !segments.isEmpty {
                                        titleCrumb(file, leading: ["…"])
                                    }

                                    titleCrumb(file, leading: [])
                                }
                                .font(.subheadline)
                                .padding(.horizontal, 16)
                                .padding(.vertical, 8)
                                .background {
                                    Capsule()
                                        .fill(titleCapsuleColor)
                                        .shadow(color: .black.opacity(0.08), radius: 3, y: 1)
                                }
                                .contentShape(Capsule())
                            }
                            .buttonStyle(.plain)
                        }
                    }

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
                                Image(systemName: "sidebar.right")
                            }
                        }
                    }
                }
                .inspector(isPresented: inspectorPresented) {
                    WorkspaceTabsList(fileTreeModel: fileTreeModel)
                        .inspectorColumnWidth(min: 220, ideal: 260, max: 320)
                }
                .sheet(item: $shareTarget) { file in
                    ShareFileSheet(id: file.id)
                }
                .sheet(item: $renameTarget) { file in
                    CreateFileSheet(fileTreeModel: fileTreeModel, mode: .rename(file))
                }
                #if os(iOS)
                    .sheet(isPresented: $showCreateAlongside) {
                        CreateFileSheet(
                            fileTreeModel: fileTreeModel,
                            mode: .create(location: openDocFile != nil ? .alongside : .root)
                        )
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
            if let hudState {
                hud(for: hudState)
                    .transition(.opacity.combined(with: .scale(scale: 0.94)))
            }
        }
        .animation(.easeInOut(duration: 0.2), value: hudState)
        #if os(iOS)
            .overlay {
                tabsDrawer
            }
            .sheet(isPresented: Binding(
                get: { workspaceOutput.openCamera },
                set: { workspaceOutput.openCamera = $0 }
            )) {
                CameraView()
                    .ignoresSafeArea()
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

            if status.outOfSpace, !hideOutOfSpaceAlert {
                showOutOfSpaceAlert = true
            }
        }
        .alert("You're out of space", isPresented: $showOutOfSpaceAlert) {
            Button("Don't show again") {
                hideOutOfSpaceAlert = true
            }

            Button("Dismiss", role: .cancel) {}
        } message: {
            Text("Your changes will stop syncing until you free up space or upgrade your account in Settings.")
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

    private enum HUDState: Equatable {
        case openingLink
        case importing
        case exporting
        case imported(ImportResult)
        case moved(MoveResult)
    }

    private var hudState: HUDState? {
        if homeState.openingLink {
            .openingLink
        } else if filesModel.importsInProgress > 0 {
            .importing
        } else if filesModel.exportsInProgress > 0 {
            .exporting
        } else if let result = filesModel.importResult {
            .imported(result)
        } else if let result = filesModel.moveResult {
            .moved(result)
        } else {
            nil
        }
    }

    @ViewBuilder
    private func hud(for state: HUDState) -> some View {
        switch state {
        case .openingLink:
            StatusHUD(message: "Opening link")
        case .importing:
            StatusHUD(message: "Importing files")
        case .exporting:
            StatusHUD(message: "Exporting files")
        case let .imported(result):
            ResultHUD(id: result, message: result.message, systemImage: result.systemImage) {
                filesModel.importResult = nil
            }
        case let .moved(result):
            ResultHUD(id: result, message: result.message, systemImage: result.systemImage) {
                filesModel.moveResult = nil
            }
        }
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
                .createGestures(trigger: quickCreateCount, tap: { showCreateAlongside = true }, longPress: quickCreateAlongside)
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

                    WorkspaceTabsList(fileTreeModel: fileTreeModel)
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

            if selectedTab == .files || selectedTab == .recents {
                PinnedFilesSection(fileTreeModel: fileTreeModel) {
                    selectedTab = .files
                }
            }

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
                            .createGestures(trigger: quickCreateCount, tap: { showCreateFile = true }, longPress: quickCreateDoc)
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
                    .createGestures(trigger: quickCreateCount, tap: { showCreateFile = true }, longPress: quickCreateDoc)

                actionChip("Import", systemImage: "square.and.arrow.down")
                    .onTapGesture { showImporter = true }

                actionChip("Search", systemImage: "magnifyingglass")
                    .onTapGesture { workspaceInput.showSearch() }
            }
            .padding(.horizontal, 10)
            .padding(.vertical, 8)
            .fileImporter(
                isPresented: $showImporter, allowedContentTypes: [.item], allowsMultipleSelection: true
            ) { result in
                guard let root = filesModel.root, case let .success(urls) = result, !urls.isEmpty else {
                    return
                }

                filesModel.importFiles(urls: urls, into: root)
            }
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

    private var titleCapsuleColor: Color {
        #if os(macOS)
            Color(nsColor: .controlBackgroundColor)
        #else
            Color(.systemBackground)
        #endif
    }

    private func titleCrumb(_ file: File, leading: [String]) -> some View {
        HStack(spacing: 8) {
            Image(systemName: FileIconHelper.docNameToSystemImageName(name: file.name))
                .font(.system(size: 13))
                .foregroundStyle(Color.accentColor)

            ForEach(Array(leading.enumerated()), id: \.offset) { _, segment in
                Text(segment)
                    .foregroundStyle(.secondary)
                    .lineLimit(1)

                Image(systemName: "chevron.compact.right")
                    .font(.system(size: 12))
                    .foregroundStyle(.tertiary)
            }

            Text(file.name)
                .fontWeight(.semibold)
                .foregroundStyle(.primary)
                .lineLimit(1)
                .truncationMode(.middle)
                .layoutPriority(1)

            Image(systemName: "pencil")
                .font(.system(size: 12, weight: .semibold))
                .foregroundStyle(.secondary)
                .padding(.leading, 4)
        }
    }

    private var detailTitle: String {
        #if os(macOS)
            openDocFile == nil ? "Lockbook" : ""
        #else
            openDocFile?.name ?? "Lockbook"
        #endif
    }

    @ViewBuilder
    private var workspace: some View {
        if isPreview {
            Text("Workspace")
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        } else {
            WorkspaceView()
        }
    }
}

private extension View {
    func createGestures(
        trigger: Int, tap: @escaping () -> Void, longPress: @escaping () -> Void
    ) -> some View {
        onTapGesture(perform: tap)
            .onLongPressGesture(perform: longPress)
            .sensoryFeedback(.impact(weight: .medium), trigger: trigger)
    }
}

enum SidebarTab: String, Identifiable {
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
