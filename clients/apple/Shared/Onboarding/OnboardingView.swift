import SwiftUI

struct OnboardingView: View {
    @ObservedObject var core: Core
    
    var body: some View {
        VStack {
            NavigationView {
                VStack(spacing: 40) {
                    NavigationLink(destination: CreateAccountView(core: self.core)) {
                        Label("Create", systemImage: "person.crop.circle.badge.plus")
                    }
                    NavigationLink(destination: ImportAccountView(core: self.core)) {
                        Label("Import", systemImage: "rectangle.stack.person.crop")
                    }
                }
                
                // For iPad and macOS
                Text("You need an account!")
            }
        }
    }
}

struct OnboardingView_Previews: PreviewProvider {
    static var previews: some View {
        /// You can point this to a real directory with:
        // OnboardingView(core: Core(documenstDirectory: "<somedir>"))
        OnboardingView(core: Core())        
    }
}
