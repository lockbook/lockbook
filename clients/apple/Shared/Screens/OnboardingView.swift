import Foundation
import SwiftUI

struct OnboardingView: View {
    var body: some View {
        NavigationStack {
            VStack(alignment: .leading) {
                HStack {
                    LogoView()
                    
                    Spacer()
                }
                                    
                Text("Lockbook")
                    .font(.largeTitle)
                    .fontWeight(.bold)
                    .padding(.leading)
                
                Text("The private note-taking platform.")
                    .font(.title2)
                    .padding(.leading)

                subText
                
                Spacer()
                
                NavigationLink(destination: {
                    OnboardingTwoView()
                }, label: {
                    Text("Get started")
                        .fontWeight(.semibold)
                        .frame(maxWidth: .infinity)
                        .frame(height: 30)
                })
                .buttonStyle(.borderedProminent)
                .padding(.bottom, 6)
                
                NavigationLink(destination: {
                    ImportAccountView()
                }, label: {
                    Text("I have an account")
                        .fontWeight(.semibold)
                        .frame(maxWidth: .infinity)
                        .frame(height: 30)
                })
                .buttonStyle(.bordered)
                .padding(.bottom)
                
                Text("By using Lockbook, you acknowledge our [Privacy Policy](https://lockbook.net/privacy-policy) and accept our [Terms of Service](https://lockbook.net/tos).")
                    .foregroundColor(.gray)
                    .font(.caption2)
            }
            .padding(.top, 35)
            .padding(.bottom)
            .modifier(OnboardingOneHorizontalPadding())
        }
    }
    
    
    
    var subText: some View {
        #if os(iOS)
        Text("The perfect place to record, sync, and share your thoughts.")
            .font(.body)
            .frame(maxWidth: 270)
            .padding(.top)
            .padding(.leading, 12)
        #else
        Text("The perfect place to record, sync, and share your thoughts.")
            .font(.body)
            .frame(maxWidth: 270)
            .padding(.top)
            .padding(.leading)
        #endif
    }
}

struct OnboardingOneHorizontalPadding: ViewModifier {
    func body(content: Content) -> some View {
        #if os(iOS)
        if UIDevice.current.userInterfaceIdiom == .phone {
            content
                .padding(.horizontal)
        } else {
            content
                .padding(.horizontal, 25)
        }
        #else
        content
            .padding(.horizontal, 25)
        #endif
    }
}

#Preview("Onboarding 1") {
    OnboardingView()
}

private struct OnboardingTwoView: View {
    @State var username: String = ""
    @State var createdAccount = false
    @State var showAccountInformation: String? = nil
    
    @State var error: String? = nil
    @State var working: Bool = false
    
    var body: some View {
        VStack(alignment: .leading) {
            Text("Create a username")
                .font(.title)
                .fontWeight(.bold)

            Text("Use letters **(A-Z)** and numbers **(0-9)**. Special characters aren’t allowed.")
                .padding(.top)
            
            Text("You cannot change your username later.")
                .padding(.top, 6)
            
            TextField("Username", text: $username)
                .textFieldStyle(.roundedBorder)
                .autocorrectionDisabled()
                .autocapitalizationDisabled()
                .onSubmit(createAccount)
                .padding(.top, 20)
            
            if let error = error {
                Text(error)
                    .foregroundStyle(.red)
                    .fontWeight(.bold)
                    .lineLimit(2, reservesSpace: false)
                    .padding(.top, 5)
            }
                        
            Button(action: {
                createAccount()
            }, label: {
                Text("Next")
                    .fontWeight(.semibold)
                    .frame(maxWidth: .infinity)
                    .frame(height: 30)
            })
            .buttonStyle(.borderedProminent)
            .disabled(username.isEmpty || working)
            .padding(.top)
            
            Spacer()
        }
        .padding(.top, 35)
        .padding(.bottom)
        .padding(.horizontal, 25)
        .navigationDestination(isPresented: $createdAccount, destination: {
            OnboardingThreeView(username: username)
        })
    }
    
    func createAccount() {
        working = true
        error = nil
        let apiUrl = AppState.LB_API_URL ?? "https://api.prod.lockbook.net"
        
        DispatchQueue.global(qos: .userInitiated).async {
            let operation = AppState.lb.createAccount(username: username, apiUrl: apiUrl, welcomeDoc: true)
            DispatchQueue.main.async {
                switch operation {
                case .success:
                    self.createdAccount = true
                case .failure(let err):
                    working = false
                    error = err.msg
                }
            }
        }

    }
}

#Preview("Onboarding 2") {
    OnboardingTwoView()
}

private struct OnboardingThreeView: View {
    let username: String
    
    @State var storedSecurely = false
    @State var working = false
    
    var body: some View {
        GeometryReader { geometry in
            ScrollView {
                VStack(alignment: .leading) {
                    
                    Text("Your account key")
                        .font(.title)
                        .fontWeight(.bold)
                        .padding(.bottom)
                                        
                    Text("This key confirms your identity and keeps your account secure. It's **confidential** and **cannot** be recovered if lost. You can always access your key in the **settings**.")
                    
                    Spacer()
                    
                    AccountPhraseView()
                    
                    Toggle(isOn: $storedSecurely, label: {
                        Text("I've stored my account key in safe place.")
                            .font(.callout)
                            .foregroundStyle(.primary)
                    })
                    .toggleStyle(iOSCheckboxToggleStyle())
                    .padding(.top)
                    .padding(.bottom)
                    
                    Button {
                        copyCompactKey()
                    } label: {
                        Text("Copy compact key")
                            .fontWeight(.semibold)
                            .frame(maxWidth: .infinity)
                            .frame(height: 30)
                    }
                    .buttonStyle(.bordered)
                    .padding(.bottom, 6)
                    
                    Button {
                        goToMainScreen()
                    } label: {
                        Text("Next")
                            .fontWeight(.semibold)
                            .frame(maxWidth: .infinity)
                            .frame(height: 30)
                    }
                    .buttonStyle(.borderedProminent)
                    .disabled(!storedSecurely || working)
                }
                .padding(.top, 35)
                .padding(.bottom)
                .padding(.horizontal, 25)
                .frame(minHeight: geometry.size.height)
            }
        }
        .navigationBarBackButtonHidden()
    }
    
    func copyCompactKey() {
        if case let .success(text) = AppState.lb.exportAccountPrivateKey() {
            ClipboardHelper.copyToClipboard(text)
        }
    }
    
    func goToMainScreen() {
        working = true
        AppState.shared.isLoggedIn = true
    }
}

#Preview("Onboarding 3") {
    OnboardingThreeView(username: "smail")
}

struct iOSCheckboxToggleStyle: ToggleStyle {
    func makeBody(configuration: Configuration) -> some View {
        HStack {
            Image(systemName: configuration.isOn ? "checkmark.square" : "square")
            
            configuration.label
        }
        .contentShape(Rectangle())
        .onTapGesture {
            configuration.isOn.toggle()
        }
    }
}

private struct ImportAccountView: View {
    @State var accountKey = ""
    @State var working = false
    @State var error: String? = nil
    
    @State var unsavedAPIURL: String = ""
    @State var apiURL: String = ""
    @State var importedAccount: Bool = false
    
    @State var showAPIURLSheet: Bool = false
    @State var showQRScanner: Bool = false
    
    @State var compactSheetHeight: CGFloat = 0
    
    var body: some View {
        VStack(alignment: .leading) {
            Text("Enter your key")
                .font(.title)
                .fontWeight(.bold)
            
            Text("Enter your phrase or private key, or scan your key QR from another device.")
                .padding(.top)
            
            Text("If you enter a phrase, please separate each word by a space or comma.")
                .padding(.top, 3)
                .padding(.bottom)
            
            HStack {
                SecureField("Phrase or compact key", text: $accountKey)
                    .disableAutocorrection(true)
                    .autocapitalizationDisabled()
                    .padding(.trailing, 10)
                    .textFieldStyle(.roundedBorder)
                    .onSubmit {
                        importAccount(isAutoImporting: false)
                    }
                    .onChange(of: accountKey) { _ in
                        importAccount(isAutoImporting: true)
                    }

                qrScanner
            }
            .padding(.top)
            
            if let error = error {
                Text(error)
                    .foregroundStyle(.red)
                    .fontWeight(.bold)
                    .lineLimit(2, reservesSpace: false)
                    .padding(.top, 5)
            }
            
            
            Button {
                importAccount(isAutoImporting: false)
            } label: {
                Text("Next")
                    .fontWeight(.semibold)
                    .frame(maxWidth: .infinity)
                    .frame(height: 30)
            }
            .buttonStyle(.borderedProminent)
            .padding(.top)
            .disabled(accountKey.isEmpty || working)
            
            Spacer()
            
            HStack {
                Spacer()
                
                Button(action: {
                    showAPIURLSheet = true
                }, label: {
                    Text("Advanced")
                        .font(.body)
                        .foregroundStyle(.gray)
                })
                .padding(.trailing)
                .background(apiURLSheet)
            }
        }
        .padding(.top, 35)
        .padding(.bottom)
        .padding(.horizontal, 25)
        .navigationDestination(isPresented: $importedAccount, destination: {
            ImportAccountSyncView()
        })
    }
    
    var apiURLSheet: some View {
        #if os(iOS)
        EmptyView()
            .optimizedSheet(isPresented: $showAPIURLSheet, compactSheetHeight: $compactSheetHeight, width: 500, height: 160) {
                SetAPIURLView(apiURL: $apiURL, unsavedAPIURL: apiURL)
            }
        #else
        EmptyView()
            .sheet(isPresented: $showAPIURLSheet) {
                SetAPIURLView(apiURL: $apiURL, unsavedAPIURL: apiURL)
                    .frame(width: 300, height: 140)
            }
        #endif
    }
    
    var qrScanner: some View {
        #if os(iOS)
        Button(action: {
            showQRScanner = true
        }, label: {
            Image(systemName: "qrcode.viewfinder")
                .font(.title)
                .foregroundStyle(Color.accentColor)
        })
        .sheet(isPresented: $showQRScanner) {
            CodeScannerView(codeTypes: [.qr], simulatedData: "This is simulated data", completion: handleScan)
        }
        #else
        EmptyView()
        #endif
    }
    
    func importAccount(isAutoImporting: Bool) {
        working = true
        let apiUrl: String? = if apiURL == "" {
            nil
        } else {
            apiURL
        }
        
        DispatchQueue.global(qos: .userInitiated).async {
            let res = AppState.lb.importAccount(key: accountKey, apiUrl: apiUrl)
            DispatchQueue.main.async {
                working = false
                
                switch res {
                case .success:
                    working = false
                    importedAccount = true
                case .failure(let err):
                    if !isAutoImporting {
                        error = err.msg
                    }
                }
            }
        }
    }
    
    #if os(iOS)
    func handleScan(result: Result<String, CodeScannerView.ScanError>) {
        showQRScanner = false
        switch result {
        case .success(let key):
            accountKey = key
        case .failure(_):
            error = "Could not scan account key QR."
        }
    }
    #endif
}

struct SetAPIURLView: View {
    @Binding var apiURL: String

    @State var unsavedAPIURL = ""
    @FocusState var focused: Bool
    let defaultAPIURL: String = AppState.LB_API_URL ?? "https://api.prod.lockbook.net"
    
    @Environment(\.dismiss) private var dismiss
    
    var body: some View {
        VStack(spacing: 10) {
            HStack {
                Text("API URL")
                    .bold()
                
                Spacer()
            }
            
            TextField("\(defaultAPIURL)", text: $unsavedAPIURL)
                .textFieldStyle(.roundedBorder)
                .autocorrectionDisabled()
                .autocapitalizationDisabled()
                .focused($focused)
                .onAppear {
                    focused = true
                }
                .onSubmit {
                    apiURL = unsavedAPIURL
                    dismiss()
                }
                .padding(.bottom, 20)
            
            Button {
                apiURL = unsavedAPIURL
                dismiss()
            } label: {
                Text("Save")
                    .frame(maxWidth: .infinity)
            }
            .buttonStyle(.bordered)
        }
        .padding(.horizontal)
        .padding(.vertical, 3)
        .presentationDetents([.height(110)])
        .onDisappear {
            unsavedAPIURL = ""
        }
    }
}

#Preview("Import Account") {
    ImportAccountView()
}

struct ImportAccountSyncView: View {
    @StateObject var model = ImportAccountSyncViewModel()
    
    var body: some View {
        VStack(spacing: 20) {
            Spacer()
            
            if let error = model.error {
                Text(error)
                    .foregroundColor(.red)
                
                Spacer()
                
                Button {
                    model.sync()
                } label: {
                    Text("Retry")
                        .fontWeight(.semibold)
                        .frame(maxWidth: .infinity)
                        .frame(height: 30)
                }
                .buttonStyle(.bordered)
            } else {
                ProgressView(value: model.syncProgress)
                    .frame(maxWidth: 700)
                
                Text(model.syncMsg)
                    .foregroundColor(.secondary)
            }
            
            Spacer()
        }
        .padding(.top, 35)
        .padding(.bottom)
        .padding(.horizontal, 25)
        .navigationBarBackButtonHidden()
    }
}

class ImportAccountSyncViewModel: ObservableObject {
    @Published var syncMsg: String = "..."
    @Published var syncProgress: Float = 0
    
    @Published var error: String? = nil
    
    init() {
        sync()
    }
    
    func sync() {
        DispatchQueue.global(qos: .userInteractive).async {
            let result = AppState.lb.sync { total, progress, id, msg in
                DispatchQueue.main.async {
                    self.syncProgress = Float(progress) / Float(total)
                    self.syncMsg = msg
                }
            }
            
            DispatchQueue.main.async {
                switch result {
                case .success(_):
                    AppState.shared.isLoggedIn = true
                case .failure(let err):
                    self.error = err.msg
                }
            }
        }
    }
    
}


#Preview("Import Account Sync") {
    ImportAccountSyncView()
}
