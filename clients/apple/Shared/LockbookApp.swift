import SwiftUI
import SwiftLockbookCore

@main
struct LockbookApp: App {
    @StateObject var core = Core(documenstDirectory: FileManager.default.urls(for: .documentDirectory, in: .userDomainMask).last!.path)
    
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
