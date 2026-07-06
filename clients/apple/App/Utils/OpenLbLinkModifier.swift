import SwiftUI
import SwiftWorkspace

struct OpenLbLinkModifier: ViewModifier {
    static let trustedHost = "app.lockbook.net"
    static let openRoute = "open"
    static let sharedFilesHost = "sharedFiles"
    static let appGroupId = "group.app.lockbook"

    @Environment(\.openURL) private var openURL

    @Environment(FilesModel.self) private var filesModel
    @Environment(HomeState.self) private var homeState
    @Environment(WorkspaceInputState.self) private var workspaceInput
    @Environment(WorkspaceOutputState.self) private var workspaceOutput

    func body(content: Content) -> some View {
        content
            .onOpenURL { url in
                open(url)
            }
            .onContinueUserActivity(NSUserActivityTypeBrowsingWeb) { activity in
                if let url = activity.webpageURL {
                    open(url)
                }
            }
            .onChange(of: workspaceOutput.urlsOpened) { _, urls in
                guard !urls.isEmpty else {
                    return
                }

                for url in urls {
                    open(url)
                }

                workspaceOutput.urlsOpened = []
            }
    }

    private func open(_ url: URL) {
        if url.scheme == "lb" {
            if url.host() == Self.sharedFilesHost {
                importSharedFiles(url)
                return
            }

            guard let host = url.host(), let id = UUID(uuidString: host) else {
                AppState.shared.error = .custom(title: "Could not open link", msg: "Invalid link")
                return
            }

            syncAndOpen(id: id)
            return
        }

        if url.host() == Self.trustedHost,
           url.pathComponents.count > 2,
           url.pathComponents[1] == Self.openRoute,
           let id = UUID(uuidString: url.pathComponents[2])
        {
            syncAndOpen(id: id)
            return
        }

        openURL(url)
    }

    private func importSharedFiles(_ url: URL) {
        guard let query = url.query(),
              let container = FileManager.default.containerURL(
                  forSecurityApplicationGroupIdentifier: Self.appGroupId
              )
        else {
            return
        }

        let urls = query.components(separatedBy: ",").compactMap { component in
            component.removingPercentEncoding.map {
                container.appendingPathComponent($0)
            }
        }

        guard !urls.isEmpty else {
            return
        }

        importWhenRootLoads(urls)
    }

    private func importWhenRootLoads(_ urls: [URL]) {
        if let root = filesModel.root {
            filesModel.importFiles(urls: urls, into: root)
            return
        }

        DispatchQueue.main.asyncAfter(deadline: .now() + 0.2) {
            importWhenRootLoads(urls)
        }
    }

    private func syncAndOpen(id: UUID) {
        homeState.openingLink = true

        DispatchQueue.global(qos: .userInitiated).async {
            let syncResult = AppState.lb.sync()
            let file = AppState.lb.getFile(id: id)

            DispatchQueue.main.async {
                homeState.openingLink = false
                filesModel.loadFiles()

                switch file {
                case let .success(file):
                    workspaceInput.openFile(id: file.id)
                    homeState.compactColumn = .detail
                case .failure:
                    if case let .failure(err) = syncResult {
                        AppState.shared.error = .lb(error: err)
                    } else {
                        AppState.shared.error = .custom(
                            title: "Could not open link", msg: "File not found"
                        )
                    }
                }
            }
        }
    }
}
