import SwiftUI

struct OnboardingView: View {
    
    @ObservedObject var core: GlobalState
    
    @Environment(\.horizontalSizeClass) var horizontal
    @Environment(\.verticalSizeClass) var vertical
    
    var small: Bool {
        horizontal == .compact || vertical == .compact
    }
    
    var body: some View {
        if small {
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
            .navigationBarTitle("")
            .navigationBarHidden(true)
        } else {
            VStack(spacing: 50) {
                Text("Lockbook")
                    .font(.system(.largeTitle, design: .monospaced))
                    .padding()
                HStack {
                    CreateAccountView(core: self.core)
                    Divider().frame(height: 300)
                    ImportAccountView(core: self.core)
                }
            }
        }
    }
}

struct OnboardingView_Previews: PreviewProvider {
    static var previews: some View {
        /// You can point this to a real directory with:
        // OnboardingView(core: Core(documenstDirectory: "<somedir>"))
        OnboardingView(core: GlobalState())
    }
}
