import SwiftUI

struct OnboardingView: View {
    
    @ObservedObject var core: GlobalState
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
                        NavigationLink(destination: CreateAccountView(core: core, onboardingState: onboardingState)) {
                            Label("Create", systemImage: "person.crop.circle.badge.plus")
                        }
                        NavigationLink(destination: ImportAccountView(core: self.core, onboardingState: onboardingState)) {
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
                        CreateAccountView(core: self.core, onboardingState: onboardingState)
                        Divider().frame(height: 300)
                        ImportAccountView(core: self.core, onboardingState: onboardingState)
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
        OnboardingView(core: GlobalState(), onboardingState: OnboardingState(core: GlobalState()))
    }
}

struct Syncing_Previews: PreviewProvider {
    static var onboardingState = OnboardingState(core: GlobalState())
    
    static var previews: some View {
        /// You can point this to a real directory with:
        // OnboardingView(core: Core(documenstDirectory: "<somedir>"))
        OnboardingView(core: GlobalState(), onboardingState: onboardingState)
            .onAppear {
                onboardingState.initialSyncing = true
            }
    }
}
