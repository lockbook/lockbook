import SwiftUI
import SwiftLockbookCore
import AlertToast

struct BookView: View {

    @EnvironmentObject var sheets: SheetState
    @EnvironmentObject var onboarding: OnboardingService
    @EnvironmentObject var files: FileService
    @EnvironmentObject var share: ShareService

    let currentFolder: File
    let account: Account
    
    #if os(iOS)
    @Environment(\.horizontalSizeClass) var horizontal
    @Environment(\.verticalSizeClass) var vertical
    #endif
    
    var body: some View {
        platformFileTree
            .iOSOnlySheet(isPresented: $sheets.moving)
            .sheet(isPresented: $onboarding.anAccountWasCreatedThisSession, content: BeforeYouStart.init)
            .sheet(isPresented: $sheets.sharingFile, content: ShareFileSheet.init)
            .sheet(isPresented: $sheets.creatingFolder, content: NewFolderSheet.init)
            .toast(isPresenting: Binding(get: { files.successfulAction != nil }, set: { _ in files.successfulAction = nil }), duration: 2, tapToDismiss: true) {
                postFileAction()
            }
    }
    
    func postFileAction() -> AlertToast {
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
    
    #if os(iOS)
    var iOS: some View {
        NavigationView {
            FileListView()
                .toolbar {
                    ToolbarItemGroup {
                        NavigationLink(
                            destination: PendingSharesView()) {
                                pendingShareToolbarIcon(isPendingSharesEmpty: share.pendingShares.isEmpty)
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
    }

    @ViewBuilder
    var iPad: some View {
        NavigationView {
            FileTreeView(currentFolder: currentFolder, account: account)
        }
    }
    #else
    var macOS: some View {
        NavigationView {
            FileListView()
        }
    }
    #endif

    @ViewBuilder
    var platformFileTree: some View {
        #if os(iOS)
        if horizontal == .regular && vertical == .regular {
            iPad
        } else {
            iOS
        }
        #else
        macOS
        #endif
    }
}

extension View {
    func iOSOnlySheet(isPresented: Binding<Bool>) -> some View {
        #if os(iOS)
        self.sheet(isPresented: isPresented, content: MoveSheet.init)
        #else
        self
        #endif
    }
    
    #if os(iOS)
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
    #endif
}

#if os(macOS)

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

struct BookView_Previews: PreviewProvider {
    static var previews: some View {
        Group {
            BookView(currentFolder: FakeApi.root, account: .fake(username: "jeff"))
                    .ignoresSafeArea()
        }
    }
}
