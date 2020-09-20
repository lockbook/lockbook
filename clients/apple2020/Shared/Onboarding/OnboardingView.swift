import SwiftUI

struct OnboardingView: View {
    @ObservedObject var core: Core
    
    var body: some View {
        VStack {
            NavigationView {
                VStack {
                    VStack {
                        NavigationLink(destination: CreateAccountView(core: self.core)) {
                            Label("Create", systemImage: "person.crop.circle.badge.plus")
                        }
                            .padding(.bottom, 40)
                        NavigationLink(destination: ImportAccountView(core: self.core)) {
                            Label("Import", systemImage: "rectangle.stack.person.crop")
                        }
                    }
                }
                .navigationTitle("Account")
                
                // For iPad and macOS
                Text("You need an account!")
            }
            self.core.message.map { MessageBanner(core: self.core, message: $0) }
        }
        .ignoresSafeArea()
    }
}

struct OnboardingView_Previews: PreviewProvider {
    static var previews: some View {
        /// This allows us to use the real Rust FFI in SwiftUI Previews!
//         OnboardingView(core: Core(documenstDirectory: "/Users/raayanpillai/ios_preview"))
        OnboardingView(core: Core())
        
        MessageBanner(core: Core(), message: Message(words: "Oof!", icon: "exclamationmark.bubble", color: .yellow))
            .previewLayout(.sizeThatFits)
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
