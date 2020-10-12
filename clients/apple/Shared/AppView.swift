import SwiftUI

struct AppView: View {
    @ObservedObject var core: Core
    var body: some View {
        let view = VStack {
            switch core.state {
            case .ReadyToUse, .Empty:
                switch core.account {
                case .none:
                    AnyView(OnboardingView(core: core))
                case .some(let account):
                    AnyView(BookView(core: core, account: account))
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
            Alert(
                title: Text("Core Error!"),
                message: core.globalError.map({ Text($0.message) }),
                dismissButton: .default(Text("Dismiss"))
            )
        }

        return view
    }
}

struct AppView_Previews: PreviewProvider {
    static var previews: some View {
        AppView(core: .init())
    }
}
