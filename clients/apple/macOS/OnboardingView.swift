import SwiftUI

struct OnboardingView: View {
    @EnvironmentObject var onboardingState: OnboardingService
    
    var body: some View {
        VStack(spacing: 50) {
            if onboardingState.initialSyncing {
                Spacer()
                HStack {
                    Spacer()
                    ProgressView()
                    Spacer()
                }
                Text("Performing initial sync...")
                    .font(.title)
                    .bold()
                Spacer()
            } else {
                Spacer()
                LogoView()
                HStack (alignment: VerticalAlignment.top) {
                    CreateAccountView()
                    Divider().frame(height: 200)
                    ImportAccountView()
                }
                Spacer()
            }
        }
    }
    
}
