import SwiftUI

struct OnboardingView: View {
    
    @EnvironmentObject var onboardingState: OnboardingService
    
    @Environment(\.horizontalSizeClass) var horizontal
    @Environment(\.verticalSizeClass) var vertical
    
    var small: Bool {
        horizontal == .compact || vertical == .compact
    }
    
    var body: some View {
        if onboardingState.initialSyncing {
            VStack(spacing: 40) {
                Spacer()
                HStack {
                    Spacer()
                    ProgressView()
                    Spacer()
                }
                Text("Performing initial sync...")
                    .bold()
                Spacer()
            }
        } else {
            if small {
                NavigationView {
                    VStack(spacing: 40) {
                        LogoView()
                        NavigationLink(destination: CreateAccountView()) {
                            Label("Create account", systemImage: "person.crop.circle")
                        }
                        NavigationLink(destination: ImportAccountView()) {
                            Label("Import account", systemImage: "square.and.arrow.down")
                        }
                    }
                    .navigationBarHidden(true)
                    .navigationBarTitle(Text("Welcome"))
                }
                            } else {
                VStack(spacing: 50) {
                    LogoView()
                    HStack {
                        CreateAccountView()
                        Divider().frame(height: 300)
                        ImportAccountView()
                    }
                }
            }
        }
    }
}

struct OnboardingView_Previews: PreviewProvider {
    static var previews: some View {
        
        OnboardingView()
            .mockDI()
    }
}

struct Syncing_Previews: PreviewProvider {
    
    static var previews: some View {
        
        OnboardingView()
            .mockDI()
            .onAppear {
                Mock.onboarding.initialSyncing = false
            }
    }
}
