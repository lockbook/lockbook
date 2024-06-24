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
        .onChange(of: horizontal) { newHorizontal in
            horizontal == .regular && vertical == .regular ? (DI.platformViewShown = .iPad) : (DI.platformViewShown = .iOS)
        }
        .onChange(of: vertical) { newVertical in
            horizontal == .regular && vertical == .regular ? (DI.platformViewShown = .iPad) : (DI.platformViewShown = .iOS)
        }
        .sheet(isPresented: $sheets.moving, content: {
            if let meta = sheets.movingInfo {
                MoveSheet(meta: meta)
            }
        })
        
    }
    
    var iOS: some View {
        ZStack {
            NavigationView {
                FileListView()
                    .toolbar {
                        ToolbarItemGroup {
                            NavigationLink(
                                destination: PendingSharesView()) {
                                    pendingShareToolbarIcon(isPendingSharesEmpty: share.pendingShares?.isEmpty ?? false)
                                }
                            
                            NavigationLink(
                                destination: SettingsView().equatable(), isActive: $onboarding.theyChoseToBackup) {
                                    Image(systemName: "gearshape.fill").foregroundColor(.blue)
                                        .padding(.horizontal, 10)
                                }
                        }
                    }
            }
            .navigationViewStyle(.stack)
            
            GeometryReader { geometry in
                NavigationView {
                    WorkspaceView(DI.workspace, DI.coreService.corePtr)
                        .equatable()
                        .toolbar {
                            ToolbarItem(placement: .navigationBarLeading) {
                                Button(action: {
                                    workspace.closeActiveTab = true
                                }) {
                                    HStack {
                                        Image(systemName: "chevron.backward")
                                            .foregroundStyle(.blue)
                                            .bold()
                                        
                                        Text(DI.accounts.account!.username)
                                            .foregroundStyle(.blue)
                                    }
                                }
                            }
                            
                            ToolbarItemGroup {
                                if let id = workspace.openDoc {
                                    if let meta = DI.files.idsAndFiles[id] {
                                        Button(action: {
                                            DI.sheets.sharingFileInfo = meta
                                        }, label: {
                                            Label("Share", systemImage: "person.wave.2.fill")
                                        })
                                        .foregroundColor(.blue)
                                        .padding(.trailing, 10)
                                        
                                        Button(action: {
                                            exportFileAndShowShareSheet(meta: meta)
                                        }, label: {
                                            Label("Share externally to...", systemImage: "square.and.arrow.up.fill")
                                        })
                                        .foregroundColor(.blue)
                                        .padding(.trailing, 10)
                                    }
                                }
                            }
                        }
                }
                .offset(x: workspace.currentTab != .Welcome ? workspace.dragOffset : geometry.size.width)
            }
        }
    }
    
    var iPad: some View {
        NavigationView {
            FileTreeView()
        }
    }
    
    #else
    
    var platform: some View {
        ZStack {
            NavigationView {
                FileListView()
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
