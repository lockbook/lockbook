import SwiftUI

struct AppView: View {
    
    @EnvironmentObject var accounts: AccountService
    @EnvironmentObject var files: FileService
    @EnvironmentObject var errors: UnexpectedErrorService
    
    @ViewBuilder
    var body: some View {
        VStack {
            if accounts.calculated {
                if accounts.account == nil {
                    #if os(macOS)
                        OnboardingView()
                    #else
                        OnboardingOneView()
                    #endif
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
            if let error = errors.globalError {
                if error.code == .clientUpdateRequired {
                    return updateAlert
                } else {
                    return Alert(
                        title: Text("Error"),
                        message: Text(error.msg),
                        dismissButton: .default(Text("Dismiss"))
                    )
                }
            } else {
                return Alert(
                    title: Text("Error"),
                    message: Text("An unknown error has occurred."),
                    dismissButton: .default(Text("Dismiss"))
                )
            }
        }
        .alert(isPresented: Binding(get: { errors.errorWithTitle != nil }, set: { _ in errors.errorWithTitle = nil })) {
            if let error = errors.errorWithTitle {
                return Alert(
                    title: Text(error.0),
                    message: Text(error.1),
                    dismissButton: .default(Text("Dismiss"))
                )
            } else {
                return Alert(
                    title: Text("Error"),
                    message: Text("An unknown error has occurred."),
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
