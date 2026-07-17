import SwiftUI
import SwiftWorkspace

struct HomeView: View {
    @Environment(\.isPreview) private var isPreview
    #if os(iOS)
        @Environment(\.horizontalSizeClass) private var horizontalSizeClass
    #else
        @Environment(\.controlActiveState) private var controlActiveState
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
    @State private var showUpgrade = false
    @State private var upgradeModel: SettingsModel? = nil
    @State private var detailWidth: CGFloat = 0
    @State private var syncPillDisplay: SyncPillDisplay? = nil
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
                .navigationSplitViewColumnWidth(min: 268, ideal: 300, max: 500)
                #if os(iOS)
                    .toolbar(removing: .sidebarToggle)
                #endif
        } detail: {
            workspace
                .navigationTitle(detailTitle)
                #if os(iOS)
                    .onGeometryChange(for: CGFloat.self) { $0.size.width } action: { detailWidth = $0 }
                    .navigationBarTitleDisplayMode(.inline)
                    .toolbar(removing: .sidebarToggle)
                #else
                    .background {
                        DetailWidthReader { detailWidth = $0 }
                    }
                #endif
                .toolbar {
                    if let file = openDocFile {
                        ToolbarItem(placement: .principal) {
                            titlePill(for: file)
                        }
                    }

                    #if os(iOS)
                        if horizontalSizeClass == .regular,
                           homeState.splitViewVisibility != .all,
                           !keyboardVisible
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

                    if let file = openDocFile, !keyboardObscuresToolbar {
                        ToolbarItem(placement: sharePlacement) {
                            Button {
                                shareTarget = file
                            } label: {
                                Image(systemName: "square.and.arrow.up")
                            }
                        }
                    }

                    if workspaceOutput.tabCount > 0, !keyboardObscuresToolbar {
                        ToolbarItem(placement: sharePlacement) {
                            Button {
                                showTabsSidebar.toggle()
                            } label: {
                                Image(systemName: "sidebar.right")
                            }
                        }
                    }

                    #if os(iOS)
                        if keyboardVisible {
                            ToolbarItem(placement: .topBarTrailing) {
                                Button {
                                    dismissKeyboard()
                                } label: {
                                    Image(systemName: "checkmark")
                                        .fontWeight(.bold)
                                        .frame(width: 26, height: 26)
                                }
                                .buttonStyle(.glassProminent)
                            }
                        }
                    #endif
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
                        if !hideFloatingButtons {
                            HStack {
                                detailSearchButton

                                Spacer()

                                detailCreateButton
                            }
                            .padding(.horizontal, 20)
                            .transition(.opacity)
                        }
                    }
                    .animation(.easeInOut(duration: 0.2), value: hideFloatingButtons)
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
        .onChange(of: AppState.lb.events.docWritten) { _, docWritten in
            guard let docWritten else { return }
            filesModel.documentWritten(docWritten.id)
        }
        .onAppear {
            workspaceInput.setSidebarOpen(nativeSidebarOpen)
        }
        .onChange(of: nativeSidebarOpen) { _, open in
            workspaceInput.setSidebarOpen(open)
        }
        .onChange(of: AppState.lb.events.status, initial: true) { _, status in
            filesModel.recomputeStatusDots(status: status)
            reconcileSyncPill()

            if status.outOfSpace, !hideOutOfSpaceAlert {
                showOutOfSpaceAlert = true
            }
        }
        #if os(iOS)
            .task(id: homeState.explicitSyncCount) {
                await runSyncPillFlow()
            }
        #endif
        .alert("You're out of space", isPresented: $showOutOfSpaceAlert) {
            Button("Upgrade") {
                upgradeModel = SettingsModel()
                showUpgrade = true
            }

            Button("Don't show again") {
                hideOutOfSpaceAlert = true
            }

            Button("Dismiss", role: .cancel) {}
        } message: {
            Text("Your changes will stop syncing until you free up space or upgrade for more storage.")
        }
        .sheet(isPresented: $showUpgrade) {
            if let upgradeModel {
                NavigationStack {
                    UpgradeAccountView(settingsModel: upgradeModel)
                        .toolbar {
                            ToolbarItem(placement: .cancellationAction) {
                                Button("Done") { showUpgrade = false }
                            }
                        }
                }
                #if os(macOS)
                    .frame(minWidth: 420, minHeight: 520)
                #endif
            }
        }
        .onChange(of: workspaceOutput.tabCount) { _, count in
            if count == 0 {
                showTabsSidebar = false
            }
        }
        .onReceive(NotificationCenter.default.publisher(for: .createNewFile)) { _ in
            #if os(macOS)
                guard controlActiveState == .key else { return }
            #endif
            showCreateFile = true
        }
        .onReceive(NotificationCenter.default.publisher(for: .toggleSidebar)) { _ in
            #if os(macOS)
                guard controlActiveState == .key else { return }
            #endif
            withAnimation {
                homeState.splitViewVisibility = homeState.splitViewVisibility == .detailOnly ? .all : .detailOnly
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

    private var nativeSidebarOpen: Bool {
        #if os(iOS)
            if horizontalSizeClass == .compact {
                return homeState.compactColumn == .sidebar
            }
        #endif

        return homeState.splitViewVisibility == .all
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

    private var keyboardObscuresToolbar: Bool {
        #if os(iOS)
            keyboardVisible
        #else
            false
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
        private var hideFloatingButtons: Bool {
            keyboardVisible || workspaceOutput.mobileToolbarShown || chatOpen
        }

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

        private var titlePillMaxWidth: CGFloat {
            guard detailWidth > 0 else { return .infinity }

            var chrome: CGFloat = 76
            if keyboardVisible {
                chrome += 58
            } else {
                if openDocFile != nil { chrome += 50 }
                if workspaceOutput.tabCount > 0 { chrome += 50 }
                if horizontalSizeClass == .regular, homeState.splitViewVisibility != .all {
                    chrome += 50
                }
            }

            return max(140, detailWidth - chrome)
        }

        private func dismissKeyboard() {
            UIApplication.shared.sendAction(
                #selector(UIResponder.resignFirstResponder), to: nil, from: nil, for: nil
            )
        }

        // The workspace's own report of what's rendered — not inferred from
        // the file name, which would re-implement the Rust side's type
        // mapping and drift.
        private var chatOpen: Bool {
            workspaceOutput.currentTab == .Chat
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

            #if os(macOS)
                SyncStatusFooter(action: syncTapped)
            #else
                if horizontalSizeClass == .regular {
                    SyncStatusFooter(action: syncTapped)
                }
            #endif
        }
    }

    private func syncTapped() {
        let status = AppState.lb.events.status

        if status.updateRequired {
            AppState.shared.error = .custom(
                title: "Your Lockbook is out of date", msg: "Update to the latest version to sync"
            )
        } else if status.outOfSpace {
            showOutOfSpaceAlert = true
        }

        workspaceInput.requestSync()
    }

    private static let syncPillAnim: Animation = .spring(response: 0.35, dampingFraction: 0.8)

    private func reconcileSyncPill() {
        switch syncPillDisplay {
        case .syncing, .synced:
            return
        default:
            break
        }

        let target = SyncPillDisplay.attentionOnly(for: AppState.lb.events.status)
            .map { SyncPillDisplay.attention($0) }

        if syncPillDisplay != target {
            withAnimation(Self.syncPillAnim) {
                syncPillDisplay = target
            }
        }
    }

    private func runSyncPillFlow() async {
        guard homeState.explicitSyncCount != 0 else { return }

        withAnimation(Self.syncPillAnim) {
            syncPillDisplay = .syncing
        }

        try? await Task.sleep(for: .milliseconds(600))
        guard !Task.isCancelled else { return }

        while AppState.lb.events.status.syncing {
            try? await Task.sleep(for: .milliseconds(100))
            guard !Task.isCancelled else { return }
        }

        if let attention = SyncPillDisplay.attentionOnly(for: AppState.lb.events.status) {
            withAnimation(Self.syncPillAnim) {
                syncPillDisplay = .attention(attention)
            }
            return
        }

        withAnimation(Self.syncPillAnim) {
            syncPillDisplay = .synced
        }

        try? await Task.sleep(for: .milliseconds(1200))
        guard !Task.isCancelled else { return }

        withAnimation(Self.syncPillAnim) {
            syncPillDisplay = SyncPillDisplay.attentionOnly(for: AppState.lb.events.status)
                .map { .attention($0) }
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
            if showTabPicker {
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
                .sharedBackgroundVisibility(.hidden)
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
                        SyncStatusPill(display: syncPillDisplay, action: syncTapped)
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

    private var showTabPicker: Bool {
        #if os(macOS)
            nativeSidebarOpen
        #else
            true
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

    private func titlePill(for file: File) -> some View {
        let variant = crumbVariant(for: file)

        return Button {
            renameTarget = file
        } label: {
            titleCrumb(file, leading: variant.leading, showIcon: variant.showIcon, showPencil: variant.showPencil)
                .font(.subheadline)
                .padding(.horizontal, 16)
                .padding(.vertical, 8)
                .background {
                    Capsule()
                        .fill(titleCapsuleColor)
                        .shadow(color: .black.opacity(0.08), radius: 3, y: 1)
                }
                .contentShape(Capsule())
                #if os(iOS)
                    .frame(maxWidth: crumbAvailableWidth)
                #endif
        }
        .buttonStyle(.plain)
        #if os(macOS)
            .cappedWidth(crumbAvailableWidth)
            .id(variant)
        #endif
    }

    private var crumbAvailableWidth: CGFloat {
        #if os(macOS)
            detailWidth > 0 ? detailWidth * 0.6 : 600
        #else
            titlePillMaxWidth
        #endif
    }

    private struct CrumbVariant: Hashable {
        var leading: [String]
        var showIcon = true
        var showPencil = true
    }

    private func crumbVariant(for file: File) -> CrumbVariant {
        let segments = filesModel.ancestors(of: file).map(\.name)

        var options: [CrumbVariant] = [CrumbVariant(leading: segments)]
        if segments.count > 1, let parent = segments.last {
            options.append(CrumbVariant(leading: ["…", parent]))
        }
        if !segments.isEmpty {
            options.append(CrumbVariant(leading: ["…"]))
        }
        options.append(CrumbVariant(leading: [], showIcon: false, showPencil: false))

        let available = crumbAvailableWidth
        for variant in options where crumbWidth(variant, name: file.name) <= available {
            return variant
        }

        return options.last ?? CrumbVariant(leading: [], showIcon: false, showPencil: false)
    }

    private func crumbWidth(_ variant: CrumbVariant, name: String) -> CGFloat {
        var width: CGFloat = 32
        if variant.showIcon { width += 23 }
        for segment in variant.leading {
            width += crumbTextWidth(segment) + 8
            width += 18
        }
        width += crumbTextWidth(name) + 8
        if variant.showPencil { width += 16 }

        return width
    }

    private func crumbTextWidth(_ string: String) -> CGFloat {
        #if os(macOS)
            let font = NSFont.preferredFont(forTextStyle: .subheadline)
        #else
            let font = UIFont.preferredFont(forTextStyle: .subheadline)
        #endif

        return (string as NSString).size(withAttributes: [.font: font]).width
    }

    private var titleCapsuleColor: Color {
        #if os(macOS)
            Color(nsColor: .controlBackgroundColor)
        #else
            Color(.systemBackground)
        #endif
    }

    private func titleCrumb(
        _ file: File, leading: [String], showIcon: Bool = true, showPencil: Bool = true
    ) -> some View {
        HStack(spacing: 8) {
            if showIcon {
                Image(systemName: FileIconHelper.docNameToSystemImageName(name: file.name))
                    .font(.system(size: 13))
                    .foregroundStyle(Color.accentColor)
            }

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

            if showPencil {
                Image(systemName: "pencil")
                    .font(.system(size: 12, weight: .semibold))
                    .foregroundStyle(.secondary)
                    .padding(.leading, 4)
            }
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
                .frame(minWidth: 5, minHeight: 5)
                .ignoresSafeArea(.keyboard)
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

#if os(macOS)
    private struct CappedWidthLayout: Layout {
        let width: CGFloat?

        func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
            subviews.first?.sizeThatFits(capped(proposal)) ?? .zero
        }

        func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
            subviews.first?.place(
                at: CGPoint(x: bounds.midX, y: bounds.midY), anchor: .center, proposal: capped(proposal)
            )
        }

        private func capped(_ proposal: ProposedViewSize) -> ProposedViewSize {
            guard let width else { return proposal }

            return ProposedViewSize(width: min(proposal.width ?? width, width), height: proposal.height)
        }
    }

    private extension View {
        func cappedWidth(_ width: CGFloat?) -> some View {
            CappedWidthLayout(width: width) { self }
        }
    }

    private struct DetailWidthReader: NSViewRepresentable {
        let onChange: (CGFloat) -> Void

        func makeNSView(context: Context) -> WidthReportingView {
            let view = WidthReportingView()
            view.onChange = onChange
            return view
        }

        func updateNSView(_ nsView: WidthReportingView, context: Context) {
            nsView.onChange = onChange
        }

        final class WidthReportingView: NSView {
            var onChange: ((CGFloat) -> Void)?
            private var lastWidth: CGFloat = 0

            override init(frame frameRect: NSRect) {
                super.init(frame: frameRect)
                postsFrameChangedNotifications = true
                NotificationCenter.default.addObserver(
                    self, selector: #selector(frameChanged),
                    name: NSView.frameDidChangeNotification, object: self
                )
            }

            @available(*, unavailable)
            required init?(coder: NSCoder) {
                fatalError("init(coder:) has not been implemented")
            }

            deinit {
                NotificationCenter.default.removeObserver(self)
            }

            override func layout() {
                super.layout()
                report()
            }

            @objc private func frameChanged() {
                report()
            }

            private func report() {
                let width = bounds.width
                guard width != lastWidth, width > 0 else { return }

                lastWidth = width
                DispatchQueue.main.async { [weak self] in
                    self?.onChange?(width)
                }
            }
        }
    }
#endif

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
