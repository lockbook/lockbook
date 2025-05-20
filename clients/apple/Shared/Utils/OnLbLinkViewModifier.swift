import SwiftUI

struct OnLbLinkViewModifier: ViewModifier {
    @EnvironmentObject var homeState: HomeState
    @EnvironmentObject var filesModel: FilesViewModel
    
    func body(content: Content) -> some View {
        content.onOpenURL(perform: { url in
            if url.scheme == "lb" {
                if url.host == "sharedFiles" {
                    handleImportLink(url: url)
                } else {
                    handleOpenLink(url: url)
                }
            }
        })
    }
    
    func handleImportLink(url: URL) {
        DispatchQueue.global(qos: .userInitiated).async {
            while filesModel.root == nil {
                Thread.sleep(until: .now + 0.1)
            }
                        
            if let filePathsQuery = url.query,
               let containerURL = FileManager.default.containerURL(forSecurityApplicationGroupIdentifier: "group.app.lockbook") {
                let filePaths = filePathsQuery.components(separatedBy: ",")
                
                var paths: [String] = []
                
                for filePath in filePaths {
                    paths.append(containerURL.appendingPathComponent(filePath.removingPercentEncoding!).path(percentEncoded: false))
                }
                
                DispatchQueue.main.async {
                    homeState.selectSheetInfo = .externalImport(paths: paths)
                }
                
            }
        }
    }
        
    func handleOpenLink(url: URL) {
        guard let uuidString = url.host, let id = UUID(uuidString: uuidString) else {
            homeState.error =  .custom(title: "Could not open link", msg: "Invalid URL")
            return
        }

        DispatchQueue.global(qos: .userInitiated).async {
            while filesModel.root == nil {
                Thread.sleep(until: .now + 1)
            }

            guard let file = filesModel.idsToFiles[id] else {
                homeState.error =  .custom(title: "Could not open link", msg: "File not found")
                return
            }
            
            DispatchQueue.main.async {
                AppState.workspaceState.requestOpenDoc(id)
            }
        }
    }

}

