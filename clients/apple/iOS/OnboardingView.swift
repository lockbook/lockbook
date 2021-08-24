import SwiftUI

struct OnboardingView: View {
    
    @EnvironmentObject var onboardingState: OnboardingState
    
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
                        Text("Lockbook").font(.system(.largeTitle, design: .monospaced))
                        NavigationLink(destination: CreateAccountView()) {
                            Label("Create", systemImage: "person.crop.circle.badge.plus")
                        }
                        NavigationLink(destination: ImportAccountView()) {
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
                Mock.onboarding.initialSyncing = true
            }
    }
}
