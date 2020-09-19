import SwiftUI

struct OnboardingView: View {
    @ObservedObject var core: Core
    
    var body: some View {
        NavigationView {
            VStack {
                NavigationLink(destination: CreateAccountView()) {
                    Label("Create", systemImage: "person.crop.circle.badge.plus")
                }
                .padding(.bottom, 40)
                NavigationLink(destination: ImportAccountView()) {
                    Label("Import", systemImage: "qrcode.viewfinder")
                }
            }
            .font(.title)
            .navigationTitle("Account")
            
            Text("You need an account!")
        }
    }
}

struct OnboardingView_Previews: PreviewProvider {
    static var previews: some View {
        /// This allows us to use the real Rust FFI in SwiftUI Previews!
        // OnboardingView(core: Core(documenstDirectory: "/Users/raayanpillai/ios_preview"))
        OnboardingView(core: Core())
    }
}

struct CreateAccountView: View {
    var body: some View {
        Text("Create")
    }
}

struct ImportAccountView: View {
    var body: some View {
        Text("Import")
    }
}
