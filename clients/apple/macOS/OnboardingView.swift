import SwiftUI

struct OnboardingView: View {
    @ObservedObject var core: GlobalState
    
    var body: some View {
        VStack {
            NavigationView {
                VStack(spacing: 40) {
                    Text("Lockbook").font(.system(.largeTitle, design: .monospaced))
                    NavigationLink(destination: CreateAccountView(core: self.core)) {
                        Label("Create", systemImage: "person.crop.circle.badge.plus")
                    }
                    NavigationLink(destination: ImportAccountView(core: self.core)) {
                        Label("Import", systemImage: "rectangle.stack.person.crop")
                    }
                }
            }
        }
    }
}
