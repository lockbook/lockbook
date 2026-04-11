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
                openUrl(url: url)
            })
            .onReceive(workspaceOutput.$urlsOpened, perform: { urls in
                for url in urls {
                    openUrl(url: url)
                }
            })
    }
    
    func openUrl(url: URL) {
        switch url.scheme {
        case "lb":
            if url.host == "sharedFiles" {
                importInternalLink(url: url)
            } else {
                openLbUrl(url: url)
            }
        case "http", "https":
            openExternalUrl(url: url)
        default:
            break
        }
    }
    
    private func openLbUrl(url: URL) {
        guard let uuidString = url.host, let id = UUID(uuidString: uuidString) else {
            AppState.shared.error = .custom(title: "Could not open link", msg: "Invalid URL")
            return
        }

        self.openFile(id: id)
    }
    
    private func openExternalUrl(url: URL) {
        guard url.pathComponents.count >= 3,
              url.pathComponents[1] == "open",
              let id = UUID(uuidString: url.pathComponents[2]) else {
            AppState.shared.error = .custom(
                title: "Could not open link",
                msg: "Invalid URL"
            )
            return
        }

        self.openFile(id: id)
    }

    private func importInternalLink(url: URL) {
        DispatchQueue.global(qos: .userInitiated).async {
            while filesModel.root == nil {
                Thread.sleep(until: .now + 0.1)
            }

            if let filePathsQuery = url.query,
               let containerURL = FileManager.default.containerURL(forSecurityApplicationGroupIdentifier: "group.app.lockbook")
            {
                let filePaths = filePathsQuery.components(separatedBy: ",")

                var urls: [URL] = []

                for filePath in filePaths {
                    let url = URL(filePath: containerURL.appendingPathComponent(filePath.removingPercentEncoding!).path(percentEncoded: false))
                    _ = url.startAccessingSecurityScopedResource()

                    urls.append(url)
                }

                DispatchQueue.main.async {
                    homeState.selectSheetInfo = .externalImport(urls: urls)
                }
            }
        }
    }

    
    private func openFile(id: UUID) {
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
