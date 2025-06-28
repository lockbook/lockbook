import SwiftUI
import SwiftWorkspace

struct StatusBarView: View {
    @Environment(\.isConstrainedLayout) var isConstrainedLayout
    
    @EnvironmentObject var homeState: HomeState
    @EnvironmentObject var filesModel: FilesViewModel
    @EnvironmentObject var workspaceState: WorkspaceState
    
    var body: some View {
        VStack {
            Divider()
            
            if filesModel.selectedFilesState.isSelectableState() {
                selectedFilesOptions
            } else {
                statusBar
            }
        }
        .frame(height: 40, alignment: .bottom)
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
            
            Text(AppState.workspaceState.statusMsg)
                .font(.callout)
                .lineLimit(1)
                .truncationMode(.tail)
                .padding(.leading)
            
            Spacer()
            
            if let root = filesModel.root {
                Button(action: {
                    self.docCreateAction {
                        filesModel.createDoc(parent: root.id, isDrawing: false)
                    }
                }) {
                    Image(systemName: "doc.badge.plus")
                        .font(.title2)
                        .foregroundColor(.accentColor)
                }
                .padding(.trailing, 5)
                
                Button(action: {
                    self.docCreateAction {
                        filesModel.createDoc(parent: root.id, isDrawing: true)
                    }
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
    
    func docCreateAction(f: () -> Void) {
        if isConstrainedLayout {
            homeState.constrainedSidebarState = .closed
        }
        
        f()
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
