import SwiftUI
import SwiftLockbookCore

struct AppView: View {
    
    @EnvironmentObject var accounts: AccountService
    @EnvironmentObject var files: FileService
    @EnvironmentObject var errors: UnexpectedErrorService
    
    @ViewBuilder
    var body: some View {
        VStack {
            switch accounts.account {
            case .none:
                OnboardingView()
            case .some(let account):
                switch files.root {
                case .some(let root):
                    BookView(currentFolder: root, account: account)
                        .onOpenURL() { url in
                            guard let uuidString = url.host, let id = UUID(uuidString: uuidString), url.scheme == "lb" else {
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
                        .handlesExternalEvents(preferring: ["lb"], allowing: ["lb"])

                case .none:
                    if files.hasRootLoaded {
                        OnboardingView()
                            .onAppear {
                                DI.onboarding.initialSyncing = true
                                DI.sync.importSync()
                            }
                    } else {
                        Label("Loading...", systemImage: "clock.arrow.circlepath")
                    }
                }
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
