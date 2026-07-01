import SwiftUI
import SwiftWorkspace

struct HomeView: View {
    @Environment(\.isPreview) private var isPreview

    @State private var homeState = HomeState()
    @State private var selectedTab: SidebarTab = .files

    @State private var filesModel: FilesModel
    @State private var fileTreeModel: FileTreeModel
    @State private var sharedTreeModel: FileTreeModel
    #if os(iOS)
        @State private var searchModel: SearchModel
    #endif

    @StateObject private var workspaceInput = WorkspaceInputState(coreHandle: AppState.lb.lbUnsafeRawPtr)
    @StateObject private var workspaceOutput: WorkspaceOutputState

    init() {
        let workspaceOutput = WorkspaceOutputState()
        let filesModel = FilesModel()

        _workspaceOutput = StateObject(wrappedValue: workspaceOutput)
        _filesModel = State(initialValue: filesModel)
        _fileTreeModel = State(initialValue: FileTreeModel(filesModel: filesModel, workspaceOutput: workspaceOutput))
        _sharedTreeModel = State(initialValue: FileTreeModel(filesModel: filesModel, workspaceOutput: workspaceOutput))
        #if os(iOS)
            _searchModel = State(initialValue: SearchModel(filesModel: filesModel))
        #endif
    }

    var body: some View {
        @Bindable var homeState = homeState

        NavigationSplitView(
            columnVisibility: homeState.splitViewVisibility,
            preferredCompactColumn: $homeState.compactColumn
        ) {
            sidebar
                .navigationSplitViewColumnWidth(min: 250, ideal: 300)
        } detail: {
            NavigationStack {
                workspace
            }
        }
        .environment(homeState)
        .environment(filesModel)
        .environmentObject(workspaceInput)
        .environmentObject(workspaceOutput)
    }

    private var sidebar: some View {
        Group {
            switch selectedTab {
            case .files:
                FileTreeView(fileTreeModel: fileTreeModel)
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
                    ForEach(SidebarTab.allCases) { tab in
                        Label(tab.title, systemImage: tab.systemImage)
                            .tag(tab)
                    }
                }
                .pickerStyle(.segmented)
                .fixedSize()
            }
        }
    }

    private var tabstripPlacement: ToolbarItemPlacement {
        #if os(macOS)
            .principal
        #else
            .topBarLeading
        #endif
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

enum SidebarTab: CaseIterable, Identifiable {
    case files
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
        case .sharedWithMe: "Shared"
        #if os(iOS)
            case .search: "Search"
        #endif
        }
    }

    var systemImage: String {
        switch self {
        case .files: "folder.fill"
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
