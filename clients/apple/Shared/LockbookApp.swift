import SwiftUI
import SwiftLockbookCore

@main
struct LockbookApp: App {
    @StateObject var core = Core(documenstDirectory: FileManager.default.urls(for: .documentDirectory, in: .userDomainMask).last!.path)
    
    var body: some Scene {
        WindowGroup {
            VStack {
                switch core.account {
                case .none:
                    AnyView(OnboardingView(core: core))
                case .some(let account):
//                    switch core.api.getRoot() {
//                    case .success(let root):
                    AnyView(BookView(core: core, account: account))
//                    case .failure(let err):
//                        AnyView(Text("Something horrible happened! \(err.message())"))
//                    }
                }
                self.core.message.map { MessageBanner(core: self.core, message: $0) }
            }
            .ignoresSafeArea()
        }
        #if os(macOS)
        Settings {
            switch core.account {
            case .none:
                AnyView(Text("You need an account for settings!").padding(100))
            case .some(let account):
                AnyView(AccountView(core: core, account: account).padding(100))
            }
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

struct MessageBanner: View {
    @ObservedObject var core: Core
    let message: Message
    
    var body: some View {
        HStack {
            Spacer()
            Label(message.words, systemImage: message.icon ?? "")
                .font(.headline)
                .foregroundColor(.black)
                .padding(.vertical, 20)
            Spacer()
        }
        .background(message.color)
        .onTapGesture {
            withAnimation {
                self.core.message = nil
            }
        }
    }
}
