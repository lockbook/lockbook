import SwiftUI
import SwiftLockbookCore

struct AppView: View {
    @ObservedObject var core: GlobalState
    var body: some View {
        let view = VStack {
            switch core.state {
            case .ReadyToUse, .Empty:
                switch core.account {
                case .none:
                    AnyView(OnboardingView(core: core, onboardingState: OnboardingState(core: core)))
                case .some(let account):
                    switch core.root {
                    case .some(let root):
                        AnyView(BookView(core: core, currentFolder: root, account: account))
                            .onAppear {
                                if core.account != nil {
                                    core.syncing = true
                                }
                            }
                    case .none:
                        Label("Please sync!", systemImage: "arrow.right.arrow.left.circle.fill")
                    }
                }
            case .MigrationRequired:
                AnyView(VStack(spacing: 20) {
                    Text(core.state.rawValue)
                        .foregroundColor(.yellow)
                        .bold()
                    Button(action: core.migrate) {
                        Label("Migrate", systemImage: "tray.2.fill")
                    }
                }.padding(100))
            case .StateRequiresClearing:
                AnyView(VStack(spacing: 20) {
                    Text(core.state.rawValue)
                        .foregroundColor(.red)
                        .bold()
                    Button(action: core.purge) {
                        Label("Purge", systemImage: "trash.fill")
                    }
                }.padding(100))
            }
        }
        .alert(isPresented: Binding(get: { core.globalError != nil }, set: { _ in core.globalError = nil })) {
            // TODO: Improve the UX of this
            switch core.globalError {
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
                    message: core.globalError.map({ Text($0.message) }),
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
        AppView(core: .init())
    }
}
