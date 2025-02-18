import SwiftUI
import SwiftWorkspace

struct StatusBarView: View {
    @EnvironmentObject var homeState: HomeState
    
    @ObservedObject var filesModel: FilesViewModel
    @ObservedObject var workspaceState: WorkspaceState
    
    var body: some View {
        VStack {
            Divider()

            HStack {
                if workspaceState.syncing {
                    ProgressView()
                } else {
                    Button(action: {
                        workspaceState.requestSync()
                    }) {
                        Image(systemName: "arrow.triangle.2.circlepath.circle.fill")
                            .imageScale(.large)
                            .foregroundColor(.accentColor)
                    }
                }
                
                Text(workspaceState.statusMsg)
                    .font(.callout)
                    .lineLimit(1)
                    .truncationMode(.tail)
                    .padding(.leading)
                
                Spacer()
                
                if let root = filesModel.root {
                    Button(action: {
                        filesModel.createDoc(parent: root.id, isDrawing: false)
                    }) {
                        Image(systemName: "doc.badge.plus")
                            .font(.title2)
                            .foregroundColor(.accentColor)
                    }
                    .padding(.trailing, 5)
                    
                    Button(action: {
                        filesModel.createDoc(parent: root.id, isDrawing: false)
                    }) {
                        Image(systemName: "pencil.tip.crop.circle.badge.plus")
                            .font(.title2)
                            .foregroundColor(.accentColor)
                    }
                    .padding(.trailing, 2)
                    
                    Button(action: {
                        homeState.sheetInfo = .createFolder(parent: root)
                    }) {
                        Image(systemName: "folder.badge.plus")
                            .font(.title2)
                            .foregroundColor(.accentColor)
                    }
                } else {
                    ProgressView()
                }
            }
            .padding(.horizontal, 20)
        }
        .frame(height: 40, alignment: .bottom)
    }
}

#Preview {
    let workspaceState = WorkspaceState()
    workspaceState.statusMsg = "You have 1 unsynced change."
    
    return VStack {
        Spacer()
                
        StatusBarView(filesModel: FilesViewModel(workspaceState: workspaceState), workspaceState: workspaceState)
            .environmentObject(HomeState())
    }
}
