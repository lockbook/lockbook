import SwiftUI
import SwiftWorkspace

struct StatusBarView: View {
    @EnvironmentObject var homeState: HomeState
    @EnvironmentObject var filesModel: FilesViewModel
    @EnvironmentObject var workspaceState: WorkspaceState
    
    var body: some View {
        HStack {
            SyncButton()
            
            Spacer()
            
            fileActionButtons
        }
        .padding(.horizontal, 16)
        .padding(.bottom, 16)
    }
    
    var fileActionButtons: some View {
        HStack {
            if let root = filesModel.root {
                Button(action: {
                    filesModel.createDoc(parent: selectedFolderOrRoot(root).id, isDrawing: false)
                }) {
                    Image(systemName: "doc.badge.plus")
                        .font(.title2)
                        .foregroundColor(.accentColor)
                }
                .padding(.trailing, 5)
                
                Button(action: {
                    filesModel.createDoc(parent: selectedFolderOrRoot(root).id, isDrawing: false)
                }) {
                    Image(systemName: "pencil.tip.crop.circle.badge.plus")
                        .font(.title2)
                        .foregroundColor(.accentColor)
                }
                .padding(.trailing, 2)
                
                Button(action: {
                    homeState.sheetInfo = .createFolder(parent: selectedFolderOrRoot(root))
                }) {
                    Image(systemName: "folder.badge.plus")
                        .font(.title2)
                        .foregroundColor(.accentColor)
                }
            } else {
                ProgressView()
            }
        }
        .buttonStyle(.borderless)
    }

    func selectedFolderOrRoot(_ root: File) -> File {
        guard let selectedFolder = AppState.workspaceState.selectedFolder else {
            return root
        }
        
        return filesModel.idsToFiles[selectedFolder] ?? root
    }
}

#Preview {
    let workspaceState = WorkspaceState()
    workspaceState.statusMsg = "Just synced!"
    
    return StatusBarView()
        .environmentObject(workspaceState)
        .environmentObject(FilesViewModel())
        .environmentObject(HomeState())
        .padding(.top, 8)
}
