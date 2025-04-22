import Foundation
import SwiftUI
import SwiftWorkspace
import AlertToast

struct PlatformView1: View {
    
    @EnvironmentObject var sheets: SheetState
    @EnvironmentObject var files: FileService
    @EnvironmentObject var share: ShareService
    @EnvironmentObject var search: SearchService
    @EnvironmentObject var workspace: WorkspaceState
        
    var body: some View {
        platform
            .sheet(isPresented: $sheets.moving, content: {
                if let action = sheets.movingInfo {
                    SelectFolderView(action: action)
                        .modifier(SelectFolderSheetViewModifer())
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
                    case .acceptedShare:
                        return AlertToast(type: .regular, title: "Accepted share")
                    }
                } else {
                    return AlertToast(type: .regular, title: "ERROR")
                }
            }
    }
    
    #if os(iOS)
    @Environment(\.horizontalSizeClass) var horizontal
    @Environment(\.verticalSizeClass) var vertical
    
    @State var sheetHeight: CGFloat = 0
    
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
    }
        
    var iOS: some View {
        ConstrainedHomeViewWrapper()
            .onAppear {
                if files.path.last?.type != .document {
                    if let openDoc = workspace.openDoc,
                        let meta = files.idsAndFiles[openDoc] {
                        files.path.append(meta)
                    }
                }
            }
            .confirmationDialog(
                "Are you sure? This action cannot be undone.",
                isPresented: $sheets.deleteConfirmation,
                titleVisibility: .visible,
                actions: {
                    if let metas = sheets.deleteConfirmationInfo {
                        DeleteConfirmationButtons(metas: metas)
                    }
                })
            .sheet(isPresented: $sheets.creatingFolder, content: {
                if let creatingFolderInfo = sheets.creatingFolderInfo {
                    CreateFolderSheet(info: creatingFolderInfo)
                        .modifier(AutoSizeSheetViewModifier(sheetHeight: $sheetHeight))
                }
            })
            .sheet(isPresented: $sheets.renamingFile, content: {
                if let renamingFileInfo = sheets.renamingFileInfo {
                    RenameFileSheet(info: renamingFileInfo)
                        .modifier(AutoSizeSheetViewModifier(sheetHeight: $sheetHeight))
                }
            })
            .sheet(isPresented: $sheets.sharingFile, content: {
                if let file = sheets.sharingFileInfo {
                    ShareFileSheet(file: file)
                        .modifier(AutoSizeSheetViewModifier(sheetHeight: $sheetHeight))
                }
            })
            .sheet(isPresented: $sheets.tabsList, content: {
                VStack {
                    Button(action: {
                        sheets.tabsList = false
                        files.path.removeLast()
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
                        .padding(.top, 5)
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
                                    Image(systemName: FileService.metaToSystemImage(meta: meta))
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
                        self.sheetHeight = height
                    }
                }
                .presentationDetents([.height(self.sheetHeight)])
                .presentationDragIndicator(.visible)
            })
    }
        
    var iPad: some View {
        HomeView()
            .alert("Are you sure? This action cannot be undone.",
                   isPresented: Binding(get: { sheets.deleteConfirmation && sheets.deleteConfirmationInfo != nil && sheets.deleteConfirmationInfo!.count > 1 }, set: { pres in sheets.deleteConfirmation = pres }), actions: {
                if let metas = sheets.deleteConfirmationInfo {
                    DeleteConfirmationButtons(metas: metas)
                }
            })
            .background( // to prevent force refresh of HomeView incurred by FormSheet activation
                EmptyView()
                    .modifier(FormSheetViewModifier(show: $sheets.creatingFolder, sheetContent: {
                        CreateFolderSheet(info: sheets.creatingFolderInfo!)
                            .padding(.bottom, 3)
                            .frame(width: 420, height: 190)
                    }))
                    .modifier(FormSheetViewModifier(show: $sheets.renamingFile, sheetContent: {
                        RenameFileSheet(info: sheets.renamingFileInfo!)
                            .padding(.bottom, 3)
                            .frame(width: 420, height: 190)
                    }))
                    .modifier(FormSheetViewModifier(show: $sheets.sharingFile, sheetContent: {
                        ShareFileSheet(file: sheets.sharingFileInfo!)
                            .padding(.bottom, 3)
                            .frame(width: 500, height: 355)
                    }))
            )
    }
    
    #else
    
    var platform: some View {
        ZStack {
            DesktopHomeView()
                .alert(
                    "Are you sure?",
                    isPresented: $sheets.deleteConfirmation
                ) {
                    if let metas = sheets.deleteConfirmationInfo {
                        DeleteConfirmationButtons(metas: metas)
                    }
                } message: {
                    Text("This action cannot be undone.")
                }
                .sheet(isPresented: $sheets.creatingFolder, content: {
                    if let creatingFolderInfo = sheets.creatingFolderInfo {
                        CreateFolderSheet(info: creatingFolderInfo)
                            .padding(.bottom, 3)
                            .frame(width: 300, height: 160)
                    }
                })
                .sheet(isPresented: $sheets.renamingFile, content: {
                    if let renamingFileInfo = sheets.renamingFileInfo {
                        RenameFileSheet(info: renamingFileInfo)
                            .padding(.bottom, 3)
                            .frame(width: 300, height: 160)
                    }
                })
                .sheet(isPresented: $sheets.sharingFile, content: {
                    ShareFileSheet(file: sheets.sharingFileInfo!)
                        .padding(.bottom, 3)
                        .frame(width: 430, height: 300)
                })

            
            if search.isPathSearching {
                PathSearchActionBar()
            }
        }
    }
    
    #endif
}

#if os(iOS)
extension View {
    func exportFilesAndShowShareSheet(metas: [File]) {
        DispatchQueue.global(qos: .userInitiated).async {
            var urls = []
            
            for meta in metas {
                if let url = DI.importExport.exportFilesToTempDirSync(meta: meta) {
                    urls.append(url)
                }
            }
            
            DispatchQueue.main.async {
                let activityVC = UIActivityViewController(activityItems: urls, applicationActivities: nil)
                
                if UIDevice.current.userInterfaceIdiom == .pad {
                    let thisViewVC = UIHostingController(rootView: self)
                    activityVC.popoverPresentationController?.sourceView = thisViewVC.view
                }
                
                UIApplication.shared.connectedScenes.flatMap {($0 as? UIWindowScene)?.windows ?? []}.first {$0.isKeyWindow}?.rootViewController?.present(activityVC, animated: true, completion: nil)
            }
        }
    }
}

#else
extension NSView {
    func exportFilesAndShowShareSheet(metas: [File]) {
        DispatchQueue.global(qos: .userInitiated).async {
            var urls = []
            
            for meta in metas {
                if let url = DI.importExport.exportFilesToTempDirSync(meta: meta) {
                    urls.append(url)
                }
            }

            DispatchQueue.main.async {
                NSSharingServicePicker(items: urls).show(relativeTo: .zero, of: self, preferredEdge: .minX)
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

struct SelectFolderSheetViewModifer: ViewModifier {
    func body(content: Content) -> some View {
        #if os(iOS)
        content
            .presentationDragIndicator(.visible)
            .presentationDetents([.fraction(0.8), .large])
        #else
        content.frame(width: 500, height: 500)
        #endif
    }
}

enum PlatformViewShown {
    case iOS
    case iPad
    case macOS
    case unspecifiedMobile
}
