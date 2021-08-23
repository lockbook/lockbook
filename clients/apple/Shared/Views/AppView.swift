import SwiftUI
import SwiftLockbookCore

struct AppView: View {
    
    @EnvironmentObject var dbState: DbStateService
    @EnvironmentObject var accounts: AccountService
    @EnvironmentObject var files: FileService
    @EnvironmentObject var sync: SyncService
    @EnvironmentObject var errors: UnexpectedErrorService
    
    var body: some View {
        let view = VStack {
            if let state = dbState.dbState {
                
                switch state {
                
                case .ReadyToUse, .Empty:
                    switch accounts.getAccount() {
                    case .none:
                        AnyView(OnboardingView())
                    case .some(let account):
                        switch files.root {
                        case .some(let root):
                            AnyView(BookView(currentFolder: root, account: account))
                                .onAppear {
                                    sync.syncing = true
                                }
                        case .none:
                            Label("Please sync!", systemImage: "arrow.right.arrow.left.circle.fill")
                        }
                    }
                case .MigrationRequired:
                    AnyView(VStack(spacing: 20) {
                        Text(DbState.MigrationRequired.rawValue)
                            .foregroundColor(.yellow)
                            .bold()
                        Button(action: dbState.migrate) {
                            Label("Migrate", systemImage: "tray.2.fill")
                        }
                    }.padding(100))
                case .StateRequiresClearing:
                    AnyView(VStack(spacing: 20) {
                        Text(DbState.StateRequiresClearing.rawValue)
                            .foregroundColor(.red)
                            .bold()
                        Button(action: {print("TODO")}) { // TODO
                            Label("Purge", systemImage: "trash.fill")
                        }
                    }.padding(100))
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
        message: Text("It seems like you're using an out-date client. Please update to perform sync operations."),
        dismissButton: .default(Text("Dismiss"))
    )
}

struct AppView_Previews: PreviewProvider {
    static var previews: some View {
        AppView().mockDI()
    }
}
