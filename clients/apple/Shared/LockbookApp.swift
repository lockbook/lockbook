import SwiftUI
import SwiftLockbookCore

@main
struct LockbookApp: App {
    #if os(macOS)
    @StateObject var core = Core(documenstDirectory: FileManager.default.homeDirectoryForCurrentUser.path + "/.lockbook")
    #else
    @StateObject var core = Core(documenstDirectory: FileManager.default.urls(for: .documentDirectory, in: .userDomainMask).last!.path)
    #endif
    
    var body: some Scene {
        #if os(macOS)
        WindowGroup {
            VStack {
                switch core.account {
                case .none:
                    AnyView(OnboardingView(core: core))
                case .some(let account):
                    AnyView(BookView(core: core, account: account))
                }
            }
            .alert(isPresented: Binding(get: { core.globalError != nil }, set: { _ in core.globalError = nil })) {
                // TODO: Improve the UX of this
                Alert(
                    title: Text("Core Error!"),
                    message: core.globalError.map({ Text($0.rawValue) }),
                    dismissButton: .default(Text("Dismiss"))
                )
            }
            .buttonStyle(PlainButtonStyle())
            .ignoresSafeArea()
        }
        
        Settings {
            switch core.account {
            case .none:
                AnyView(Text("You need an account for settings!").padding(100))
            case .some(let account):
                AnyView(AccountView(core: core, account: account).buttonStyle(PlainButtonStyle()).padding(100))
            }
        }
        #else
        WindowGroup {
            VStack {
                switch core.account {
                case .none:
                    AnyView(OnboardingView(core: core))
                case .some(let account):
                    AnyView(BookView(core: core, account: account))
                }
            }
            .alert(isPresented: Binding(get: { core.globalError != nil }, set: { _ in core.globalError = nil })) {
                // TODO: Improve the UX of this
                Alert(
                    title: Text("Core Error!"),
                    message: core.globalError.map({ Text($0.rawValue) }),
                    dismissButton: .default(Text("Dismiss"))
                )
            }
            .ignoresSafeArea()
        }
        #endif
    }
}

extension View {
    func hideKeyboard() {
        #if os(iOS)
        UIApplication.shared.sendAction(#selector(UIResponder.resignFirstResponder), to: nil, from: nil, for: nil)
        #endif
    }
}
