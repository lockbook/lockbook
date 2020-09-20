import SwiftUI
import SwiftLockbookCore

@main
struct LockbookApp: App {
    @StateObject var core = Core(documenstDirectory: FileManager.default.urls(for: .documentDirectory, in: .userDomainMask).last!.path)
    
    var body: some Scene {
        switch core.account {
        case .none:
            return WindowGroup {
                AnyView(OnboardingView(core: core))
            }
        case .some(let account):
            return WindowGroup {
                AnyView(VStack {
                    Text("Hello \(account.username)!")
                        .font(.title)
                        .padding(.bottom, 40)
                    Button(action: self.core.purge) {
                        Label("Purge", systemImage: "person.crop.circle.badge.xmark")
                    }
                })
            }
        }
    }
}

extension View {
    func hideKeyboard() {
        #if os(iOS)
        UIApplication.shared.sendAction(#selector(UIResponder.resignFirstResponder), to: nil, from: nil, for: nil)
        #endif
    }
}
