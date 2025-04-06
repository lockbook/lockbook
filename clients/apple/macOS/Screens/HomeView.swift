import SwiftUI
import SwiftWorkspace

struct HomeView: View {
    @StateObject var homeState = HomeState()

    var body: some View {
        PathSearchContainerView {
            NavigationSplitView(sidebar: {
                SidebarView()
            }, detail: {
                DetailView()
            })
        }
        .environmentObject(homeState)
    }
}

struct SidebarView: View {
    @EnvironmentObject var homeState: HomeState
    
    @StateObject var filesModel = FilesViewModel()

    var body: some View {
        if let error = filesModel.error {
            Text(error)
                .foregroundStyle(.red)
        } else if filesModel.loaded {
            Form {
                Section(header: Label("Suggested Documents", systemImage: "sparkle").bold().padding(.horizontal).font(.callout)) {
                    SuggestedDocsView(filesModel: filesModel)
                }
                
                Section(header: Label("Files", systemImage: "folder").bold().padding(.horizontal).font(.callout)) {
                    
                }
                
                Spacer()
            }
            .formStyle(.columns)
        }
    }
}

struct DetailView: View {
    @Environment(\.isPreview) var isPreview
    
    @EnvironmentObject var homeState: HomeState
    @EnvironmentObject var workspaceState: WorkspaceState
    
    var body: some View {
        if isPreview {
            Text("This is a preview.")
        } else {
            WorkspaceView(AppState.workspaceState, AppState.lb.lbUnsafeRawPtr)
                .toolbar {
                    HStack(alignment: .bottom, spacing: 5) {
                        if workspaceState.openDoc != nil {
                            Button(action: {
                                runOnOpenDoc { file in
                                    homeState.sheetInfo = .share(file: file)
                                }
                            }, label: {
                                Image(systemName: "person.wave.2.fill")
                            })
                            
                            Button(action: {
                                runOnOpenDoc { file in
                                    exportFiles(homeState: homeState, files: [file])
                                }
                            }, label: {
                                Image(systemName: "square.and.arrow.up.fill")
                            })
                        }
                    }
                }
        }
    }
}

#Preview("Home View") {
    let workspaceState = WorkspaceState()
    
    return HomeView()
        .environmentObject(BillingState())
        .environmentObject(workspaceState)
}
