import SwiftUI
import SwiftWorkspace

struct StatusBarView: View {
    @Environment(\.horizontalSizeClass) var horizontalSizeClass

    @EnvironmentObject var homeState: HomeState
    @EnvironmentObject var filesModel: FilesViewModel
    @EnvironmentObject var workspaceState: WorkspaceState
    
    var body: some View {
        VStack {
            if filesModel.selectedFilesState.isSelectableState() {
                selectedFilesOptions
            } else {
                statusBar
            }
        }
        .frame(height: 50, alignment: .bottom)
    }
    
    var selectedFilesOptions: some View {
        HStack(alignment: .center) {
            Spacer()
            
            Button(role: .destructive, action: {
                filesModel.deleteFileConfirmation = filesModel.getConsolidatedSelection()
            }) {
                Image(systemName: "trash")
                    .imageScale(.large)
            }
            .disabled(filesModel.selectedFilesState.count == 0)
            
            Spacer()
            
            Button(action: {
                homeState.selectSheetInfo = .move(files: filesModel.getConsolidatedSelection())
            }, label: {
                Image(systemName: "folder")
                    .imageScale(.large)
            })
            .disabled(filesModel.selectedFilesState.count == 0)
            
            Spacer()
            
            Button(action: {
                self.exportFiles(homeState: homeState, files: filesModel.getConsolidatedSelection())
                filesModel.selectedFilesState = .unselected
            }, label: {
                Image(systemName: "square.and.arrow.up")
                    .imageScale(.large)
            })
            .disabled(filesModel.selectedFilesState.count == 0)
            
            Spacer()
        }
        .foregroundStyle(filesModel.selectedFilesState.count == 0 ? .gray : Color.accentColor)
        .padding(.horizontal)
    }
    
    var statusBar: some View {
        HStack {
            SyncButton()
            
            Spacer()
            
            fileActionButtons
        }
        .padding(.horizontal, 20)
    }
    
    var fileActionButtons: some View {
        HStack {
            if let root = filesModel.root {
                Button(action: {
                    self.docCreateAction {
                        filesModel.createDoc(parent: selectedFolderOrRoot(root).id, isDrawing: false)
                    }
                }) {
                    Image(systemName: "doc.badge.plus")
                        .font(.title2)
                        .foregroundColor(.accentColor)
                }
                .padding(.trailing, 5)
                
                Button(action: {
                    self.docCreateAction {
                        filesModel.createDoc(parent: selectedFolderOrRoot(root).id, isDrawing: true)
                    }
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
    }
    
    func docCreateAction(f: () -> Void) {
        if horizontalSizeClass == .compact {
            homeState.compactSidebarState = .closed
        }
        
        f()
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
    workspaceState.statusMsg = "You have 1 unsynced change."
    
    return VStack {
        Spacer()
                
        StatusBarView()
            .environmentObject(HomeState())
            .environmentObject(FilesViewModel())
            .environmentObject(workspaceState)
    }
}
