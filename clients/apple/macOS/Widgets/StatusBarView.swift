import SwiftUI
import SwiftWorkspace

struct StatusBarView: View {
    @EnvironmentObject var homeState: HomeState
    @EnvironmentObject var filesModel: FilesViewModel
    @EnvironmentObject var workspaceInput: WorkspaceInputState
    @EnvironmentObject var workspaceOutput: WorkspaceOutputState
    
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
                    workspaceInput.createDocAt(parent: selectedFolderOrRoot(root).id, drawing: false)
                }) {
                    Image(systemName: "doc.badge.plus")
                        .font(.title2)
                        .foregroundColor(.accentColor)
                }
                .padding(.trailing, 5)
                
                Button(action: {
                    workspaceInput.createDocAt(parent: selectedFolderOrRoot(root).id, drawing: true)
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
        guard let selectedFolder = workspaceOutput.selectedFolder else {
            return root
        }
        
        return filesModel.idsToFiles[selectedFolder] ?? root
    }
}

#Preview {
    StatusBarView()
        .environmentObject(FilesViewModel())
        .environmentObject(HomeState(workspaceOutput: WorkspaceOutputState(), filesModel: FilesViewModel()))
        .padding(.top, 8)
}
