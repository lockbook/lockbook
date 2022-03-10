import SwiftUI

enum OnboardingScreen {
    case Create
    case Import
}

struct OnboardingView: View {
    
    @EnvironmentObject var onboardingState: OnboardingService
    
    @State var selectedTab: OnboardingScreen = .Create
    
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
            VStack (spacing: 30) {
                LogoView()
                Picker("", selection: $selectedTab) {
                    Text("Create").tag(OnboardingScreen.Create)
                    Text("Import").tag(OnboardingScreen.Import)
                }
                .pickerStyle(SegmentedPickerStyle())
                
                switch selectedTab {
                case .Create:
                    TextField("Choose a username: a-z, 0-9", text: self.$onboardingState.username)
                        .disableAutocorrection(true)
                        .textFieldStyle(RoundedBorderTextFieldStyle())
                        .disabled(self.onboardingState.working)
                        .onSubmit(self.onboardingState.attemptCreate)
                    Text(onboardingState.createAccountError)
                        .foregroundColor(.red)
                        .bold()
                    Button("Create Account", action: self.onboardingState.attemptCreate)
                        .foregroundColor(.blue)
                case .Import:
                    HStack {
                        SecureField("Account String", text: self.$onboardingState.accountString)
                            .disableAutocorrection(true)
                            .autocapitalization(.none)
                            .textFieldStyle(RoundedBorderTextFieldStyle())
                            .disabled(self.onboardingState.working)
                            .onSubmit(self.onboardingState.handleImport)
                        #if os(iOS)
                        QRScanner()
                        #endif
                    }
                    Text(onboardingState.importAccountError)
                        .foregroundColor(.red)
                        .bold()
                    Button("Import Account", action: self.onboardingState.handleImport)
                        .foregroundColor(.blue)
                }
            }
            .padding()
            .frame(maxWidth: 600)
        }
    }
}

struct OnboardingView_Previews: PreviewProvider {
    
    static var previews: some View {
        OnboardingView()
            .mockDI()
        OnboardingView(selectedTab: .Import)
            .mockDI()
    }
}

