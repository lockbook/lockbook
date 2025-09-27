import SwiftUI
import SwiftWorkspace

struct OnLbLinkViewModifier: ViewModifier {
    @EnvironmentObject var homeState: HomeState
    @EnvironmentObject var filesModel: FilesViewModel
    @EnvironmentObject var workspaceInput: WorkspaceInputState
    @EnvironmentObject var workspaceOutput: WorkspaceOutputState
    
    func body(content: Content) -> some View {
        content
            .onOpenURL(perform: { url in
                if url.scheme == "lb" {
                    if url.host == "sharedFiles" {
                        handleImportLink(url: url)
                    } else {
                        handleOpenLink(url: url)
                    }
                }
            })
            .onReceive(workspaceOutput.$urlOpened, perform: { url in
                guard let url else {
                    return
                }
                
                if AppState.isInternalLink(url) {
                    handleOpenLink(url: url)
                } else {
                    openURLExternally(url)
                }
            })
    }
    
    func openURLExternally(_ url: URL) {
        DispatchQueue.main.async {
            #if os(iOS)
            UIApplication.shared.open(url)
            #else
            NSWorkspace.shared.open(url)
            #endif
        }
    }
    
    func handleImportLink(url: URL) {
        DispatchQueue.global(qos: .userInitiated).async {
            while filesModel.root == nil {
                Thread.sleep(until: .now + 0.1)
            }
                        
            if let filePathsQuery = url.query,
               let containerURL = FileManager.default.containerURL(forSecurityApplicationGroupIdentifier: "group.app.lockbook") {
                let filePaths = filePathsQuery.components(separatedBy: ",")
                
                var urls: [URL] = []
                
                for filePath in filePaths {
                    let url = URL(filePath: containerURL.appendingPathComponent(filePath.removingPercentEncoding!).path(percentEncoded: false))
                    let _ = url.startAccessingSecurityScopedResource()
                    
                    urls.append(url)
                }
                
                DispatchQueue.main.async {
                    homeState.selectSheetInfo = .externalImport(urls: urls)
                }
            }
        }
    }
        
    func handleOpenLink(url: URL) {
        guard let uuidString = url.host, let id = UUID(uuidString: uuidString) else {
            AppState.shared.error =  .custom(title: "Could not open link", msg: "Invalid URL")
            return
        }

        DispatchQueue.global(qos: .userInitiated).async {
            while filesModel.root == nil {
                Thread.sleep(until: .now + 1)
            }

            guard let file = filesModel.idsToFiles[id] else {
                AppState.shared.error = .custom(title: "Could not open link", msg: "File not found")
                return
            }
            
            DispatchQueue.main.async {
                workspaceInput.openFile(id: file.id)
            }
        }
    }
}
