import SwiftUI

struct OnboardingView: View {
    
    @EnvironmentObject var core: GlobalState
    @ObservedObject var onboardingState: OnboardingState
    
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
                        NavigationLink(destination: CreateAccountView(onboardingState: onboardingState)) {
                            Label("Create", systemImage: "person.crop.circle.badge.plus")
                        }
                        NavigationLink(destination: ImportAccountView(onboardingState: onboardingState)) {
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
                        CreateAccountView(onboardingState: onboardingState)
                        Divider().frame(height: 300)
                        ImportAccountView(onboardingState: onboardingState)
                    }
                }
            }
        }
    }
}

struct OnboardingView_Previews: PreviewProvider {
    static var previews: some View {
        /// You can point this to a real directory with:
        // OnboardingView(core: Core(documenstDirectory: "<somedir>"))
        OnboardingView(onboardingState: OnboardingState(core: GlobalState()))
            .environmentObject(GlobalState())
    }
}

struct Syncing_Previews: PreviewProvider {
    static var onboardingState = OnboardingState(core: GlobalState())
    
    static var previews: some View {
        /// You can point this to a real directory with:
        // OnboardingView(core: Core(documenstDirectory: "<somedir>"))
        OnboardingView(onboardingState: onboardingState)
            .environmentObject(GlobalState())
            .onAppear {
                onboardingState.initialSyncing = true
            }
    }
}
