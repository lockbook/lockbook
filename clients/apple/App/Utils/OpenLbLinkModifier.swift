import SwiftUI
import SwiftWorkspace

struct OpenLbLinkModifier: ViewModifier {
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
                    openURL(url)
                }

                workspaceOutput.urlsOpened = []
            }
            .onChange(of: workspaceOutput.linkMessage) { _, message in
                guard let message else { return }
                workspaceOutput.linkMessage = nil
                AppState.shared.error = .custom(title: message, msg: "")
            }
    }

    /// The workspace reads every link; only shared files are the app's own.
    private func open(_ url: URL) {
        if url.scheme == "lb", url.host() == Self.sharedFilesHost {
            importSharedFiles(url)
            return
        }

        if workspaceInput.openUrl(url) {
            homeState.compactColumn = .detail
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
}
