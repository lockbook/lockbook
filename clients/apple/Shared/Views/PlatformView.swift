import Foundation
import SwiftUI
import SwiftWorkspace
import AlertToast
import SwiftLockbookCore

struct PlatformView: View {
    
    @EnvironmentObject var sheets: SheetState
    @EnvironmentObject var onboarding: OnboardingService
    @EnvironmentObject var files: FileService
    @EnvironmentObject var share: ShareService
    @EnvironmentObject var search: SearchService
    @EnvironmentObject var workspace: WorkspaceState
    
    @Environment(\.horizontalSizeClass) var horizontal
    @Environment(\.verticalSizeClass) var vertical
    
    var body: some View {
        platform
            .sheet(isPresented: $onboarding.anAccountWasCreatedThisSession, content: BeforeYouStart.init)
            .sheet(isPresented: $sheets.sharingFile, content: {
                if let meta = sheets.sharingFileInfo {
                    ShareFileSheet(meta: meta)
                }
                
            })
            .sheet(isPresented: $sheets.creatingFolder, content: {
                if let creatingFolderInfo = sheets.creatingFolderInfo {
                    CreateFolderSheet(creatingFolderInfo: creatingFolderInfo)
                }
            })
            .sheet(isPresented: $sheets.renamingFile, content: {
                if let renamingFileInfo = sheets.renamingFileInfo {
                    RenameFileSheet(renamingFileInfo: renamingFileInfo)
                }
            })
            .toast(isPresenting: Binding(get: { files.successfulAction != nil }, set: { _ in files.successfulAction = nil }), duration: 2, tapToDismiss: true) {
                if let action = files.successfulAction {
                    switch action {
                    case .delete:
                        return AlertToast(type: .regular, title: "File deleted")
                    case .move:
                        return AlertToast(type: .regular, title: "File moved")
                    case .createFolder:
                        return AlertToast(type: .regular, title: "Folder created")
                    case .importFiles:
                        return AlertToast(type: .regular, title: "Imported successfully")
                    }
                } else {
                    return AlertToast(type: .regular, title: "ERROR")
                }
            }
    }
    
    #if os(iOS)
    var platform: some View {
        Group {
            if horizontal == .regular && vertical == .regular {
                ZStack {
                    iPad
                    
                    if search.isPathSearching {
                        PathSearchActionBar()
                    }
                }
            } else {
                iOS
            }
        }
        .sheet(isPresented: $sheets.moving, content: {
            if let meta = sheets.movingInfo {
                MoveSheet(meta: meta)
            }
        })
    }
    
    @State var detentHeight: CGFloat = 0
    
    var iOS: some View {
        ConstrainedHomeViewWrapper()
            .confirmationDialog(
                "Are you sure? This action cannot be undone.",
                isPresented: $sheets.deleteConfirmation,
                titleVisibility: .visible,
                actions: {
                    if let meta = sheets.deleteConfirmationInfo {
                        DeleteConfirmationButtons(meta: meta)
                    }
                })
            .sheet(isPresented: $sheets.tabsList, content: {
                VStack {
                    Button(action: {
                        sheets.tabsList = false
                        files.path.removeLast()
                        workspace.requestCloseAllTabs()
                    }, label: {
                        HStack {
                            Image(systemName: "xmark.circle")
                                .foregroundColor(.primary)
                                .imageScale(.medium)
                                .padding(.trailing)
                                                            
                            Text("Close all tabs")
                                .foregroundColor(.primary)
                                .font(.body)
                            
                            Spacer()
                        }
                        .padding(.horizontal)
                    })
                    
                    Divider()
                        .padding(.horizontal)
                        .padding(.vertical, 3)
                    
                    ForEach(workspace.getTabsIds(), id: \.self) { id in
                        Button(action: {
                            workspace.requestOpenDoc(id)
                        }, label: {
                            if let meta = DI.files.idsAndFiles[id] {
                                HStack {
                                    Image(systemName: FileService.docExtToSystemImage(name: meta.name))
                                        .foregroundColor(.primary)
                                        .imageScale(.medium)
                                        .padding(.trailing)
                                    
                                    Text(meta.name)
                                        .foregroundColor(.primary)
                                        .font(.body)
                                        .bold(false)
                                    
                                    Spacer()
                                    
                                    if meta.id == workspace.openDoc {
                                        Image(systemName: "checkmark.circle")
                                            .foregroundColor(.primary)
                                            .font(.headline)
                                    }
                                }
                                .padding(.horizontal)
                                .padding(.vertical, 3)
                            } else {
                                Text("Loading...")
                                    .padding()
                            }
                            
                        })
                    }
                }
                .padding(.vertical)
                .modifier(ReadHeightModifier())
                .onPreferenceChange(HeightPreferenceKey.self) { height in
                    if let height {
                        self.detentHeight = height
                    }
                }
                .presentationDetents([.height(self.detentHeight)])
                .presentationDragIndicator(.visible)
            })
    }
        
    var iPad: some View {
        HomeView()
    }
    
    #else
    
    var platform: some View {
        ZStack {
            DesktopHomeView()
                .alert(
                    "Are you sure?",
                    isPresented: $sheets.deleteConfirmation
                ) {
                    if let meta = sheets.deleteConfirmationInfo {
                        DeleteConfirmationButtons(meta: meta)
                    }
                } message: {
                    Text("This action cannot be undone.")
                }

            
            if search.isPathSearching {
                PathSearchActionBar()
            }
        }
    }
    
    #endif
}

#if os(iOS)
extension View {
    func exportFileAndShowShareSheet(meta: File) {
        DispatchQueue.global(qos: .userInitiated).async {
            if let url = DI.importExport.exportFilesToTempDirSync(meta: meta) {
                DispatchQueue.main.async {
                    let activityVC = UIActivityViewController(activityItems: [url], applicationActivities: nil)
                    
                    if UIDevice.current.userInterfaceIdiom == .pad {
                        let thisViewVC = UIHostingController(rootView: self)
                        activityVC.popoverPresentationController?.sourceView = thisViewVC.view
                    }
                    
                    UIApplication.shared.connectedScenes.flatMap {($0 as? UIWindowScene)?.windows ?? []}.first {$0.isKeyWindow}?.rootViewController?.present(activityVC, animated: true, completion: nil)
                }
            }
        }
    }
}

#else
extension NSView {
    func exportFileAndShowShareSheet(meta: File) {
        DispatchQueue.global(qos: .userInitiated).async {
            if let url = DI.importExport.exportFilesToTempDirSync(meta: meta) {
                DispatchQueue.main.async {
                    NSSharingServicePicker(items: [url]).show(relativeTo: .zero, of: self, preferredEdge: .minX)
                }
            }
        }
    }
}
#endif

@ViewBuilder
func pendingShareToolbarIcon(isPendingSharesEmpty: Bool) -> some View {
    #if os(iOS)
        ZStack {
            Image(systemName: "person.2.fill")
                .foregroundColor(.blue)
                                        
            if !isPendingSharesEmpty {
                Circle()
                    .foregroundColor(.red)
                    .frame(width: 12, height: 12)
                    .offset(x: 12, y: 5)
            }
        }
    #else
        ZStack {
            Image(systemName: "person.2.fill")
                .foregroundColor(.blue)
                                        
            if !isPendingSharesEmpty {
                Circle()
                    .foregroundColor(.red)
                    .frame(width: 7, height: 7)
                    .offset(x: 7, y: 3)
            }
        }
    #endif
}

enum PlatformViewShown {
    case iOS
    case iPad
    case macOS
    case unspecifiedMobile
}
