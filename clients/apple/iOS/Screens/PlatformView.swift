import SwiftUI
import SwiftWorkspace


// avoid passing in state that is widely used, just use environment objects
struct PlatformView: View {
    @StateObject var workspaceState = WorkspaceState()
    @StateObject var errorState = ErrorState()
    
    @Environment(\.horizontalSizeClass) var horizontalSizeClass
    @Environment(\.isPreview) var isPreview
    
    var body: some View {
        if horizontalSizeClass == .compact {
            DrawerView(isOpened: true, menu: {
                sidebar
            }, content: {
                detail
            })
            .environmentObject(errorState)
            .environment(\.isConstrainedLayout, true)
        } else {
            NavigationSplitView(sidebar: {
                sidebar
            }, detail: {
                detail
            })
            .environmentObject(errorState)
            .environment(\.isConstrainedLayout, false)
        }
    }
    
    @ViewBuilder
    var sidebar: some View {
        SidebarView(workspaceState)
    }
    
    @ViewBuilder
    var detail: some View {
        if isPreview {
            Text("This is a preview.")
        } else {
            WorkspaceView(workspaceState, MainState.lb.lbUnsafeRawPtr)
        }
    }
}

struct SidebarView: View {
    @ObservedObject var workspaceState: WorkspaceState
    @ObservedObject var filesModel: FilesViewModel
    @ObservedObject var fileTreeModel: FileTreeViewModel
            
    init(_ workspaceState: WorkspaceState) {
        self.workspaceState = workspaceState
        self.filesModel = FilesViewModel(workspaceState: workspaceState)
        self.fileTreeModel = FileTreeViewModel(workspaceState: workspaceState)
    }
    
    var body: some View {
        if let error = filesModel.error {
            Text(error)
                .foregroundStyle(.red)
        } else if filesModel.loaded {
            if let root = filesModel.root {
                VStack(alignment: .leading) {
                    Section(header: Text("Suggested Docs")
                        .bold()
                        .foregroundColor(.primary)
                        .textCase(.none)
                        .font(.headline)
                        .padding(.bottom, 3)
                        .padding(.top, 8)) {
                            SuggestedDocsView(filesModel: filesModel)
                                .environmentObject(workspaceState)
                        }
                        .padding(.horizontal, 20)
                    
                    Section(header: Text("Files")
                        .bold()
                        .foregroundColor(.primary)
                        .textCase(.none)
                        .font(.headline)
                        .padding(.bottom, 3)
                        .padding(.top, 8)) {
                            FileTreeView(root: root)
                                .environmentObject(workspaceState)
                                .environmentObject(filesModel)
                                .environmentObject(fileTreeModel)
                        }
                        .padding(.horizontal, 20)
                }
                .navigationTitle(root.name)
            }
        } else {
            ProgressView()
        }
    }
}

#Preview("Platform View") {
    PlatformView()
}
