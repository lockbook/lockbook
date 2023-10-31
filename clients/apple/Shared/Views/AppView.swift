import SwiftUI
import SwiftLockbookCore

struct AppView: View {
    
    @EnvironmentObject var accounts: AccountService
    @EnvironmentObject var files: FileService
    @EnvironmentObject var sync: SyncService
    @EnvironmentObject var errors: UnexpectedErrorService
    
    var body: some View {
        let view = VStack {
            switch accounts.account {
            case .none:
                OnboardingView()
            case .some(let account):
                switch files.root {
                case .some(let root):
                    BookView(currentFolder: root, account: account)
                case .none:
                    if files.hasRootLoaded {
                        OnboardingView().onAppear {
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
        
        return view
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
