import SwiftUI

struct OnboardingView: View {
    @ObservedObject var core: GlobalState
    
    var body: some View {
        VStack {
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
