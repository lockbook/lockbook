import SwiftUI
import SwiftLockbookCore

struct AppView: View {
    
    @EnvironmentObject var accounts: AccountService
    @EnvironmentObject var files: FileService
    @EnvironmentObject var errors: UnexpectedErrorService
    
    @ViewBuilder
    var body: some View {
        VStack {
            if accounts.calculated {
                if accounts.account == nil {
                    OnboardingView()
                } else {
                    PlatformView()
                        .onOpenURL() { url in
                            if url.scheme == "lb" {
                                if url.host == "sharedFiles" {
                                    handleImportLink(url: url)
                                } else {
                                    handleOpenLink(url: url)
                                }
                            }
                        }
                        .handlesExternalEvents(preferring: ["lb"], allowing: ["lb"])
                }
            } else {
                Label("Loading...", systemImage: "clock.arrow.circlepath")
            }
        }
            .alert(isPresented: Binding(get: { errors.globalError != nil }, set: { _ in errors.globalError = nil })) {
                // TODO: Improve the UX of this
                switch errors.globalError {
                case let update as FfiError<CreateAccountError> where update == .init(.ClientUpdateRequired):
                    return updateAlert
                case let update as FfiError<ImportError> where update == .init(.ClientUpdateRequired):
                    return updateAlert
                case let update as FfiError<CalculateWorkError> where update == .init(.ClientUpdateRequired):
                    return updateAlert
                case let update as FfiError<SyncAllError> where update == .init(.ClientUpdateRequired):
                    return updateAlert
                case let update as FfiError<GetUsageError> where update == .init(.ClientUpdateRequired):
                    return updateAlert
                case let error as ErrorWithTitle:
                    return Alert(
                        title: Text(error.title),
                        message: Text(error.message),
                        dismissButton: .default(Text("Dismiss"))
                    )
                default:
                    return Alert(
                        title: Text("Core Error!"),
                        message: errors.globalError.map({ Text($0.message) }),
                        dismissButton: .default(Text("Dismiss"))
                    )
                }
            }
    }
    
    func handleImportLink(url: URL) {
        if let filePathsQuery = url.query,
           let containerURL = FileManager.default.containerURL(forSecurityApplicationGroupIdentifier: "group.app.lockbook") {
            let filePaths = filePathsQuery.components(separatedBy: ",")
            
            var res: [String] = []
            
            for filePath in filePaths {
                res.append(containerURL.appendingPathComponent(filePath.removingPercentEncoding!).path(percentEncoded: false))
            }
                                                            
            DI.sheets.movingInfo = .Import(res)
        }

    }
    
    func handleOpenLink(url: URL) {
        guard let uuidString = url.host, let id = UUID(uuidString: uuidString) else {
            DI.errors.errorWithTitle("Malformed link", "Cannot open file")
            return
        }

        DispatchQueue.global(qos: .userInitiated).async {
            while !DI.files.hasRootLoaded {
                Thread.sleep(until: .now + 1)
            }

            Thread.sleep(until: .now + 0.1)

            if DI.files.idsAndFiles[id] == nil {
                DI.errors.errorWithTitle("File not found", "That file does not exist in your lockbook")
            }

            DispatchQueue.main.async {
                DI.workspace.requestOpenDoc(id)
            }
        }

    }
    
    let updateAlert: Alert = Alert(
        title: Text("Update Required!"),
        message: Text("It seems like you're using an out-date client. Please update to continue."),
        dismissButton: .default(Text("Dismiss"))
    )
}

struct AppView_Previews: PreviewProvider {
    static var previews: some View {
        AppView().mockDI()
    }
}
