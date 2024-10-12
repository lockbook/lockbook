import Foundation
import SwiftUI

struct OnboardingOneView: View {
    var body: some View {
        NavigationStack {
            VStack(alignment: .leading) {
                HStack {
                    Image(uiImage: UIImage(named: "logo")!)
                        .resizable()
                        .scaledToFit()
                        .frame(width: 75)
                    
                    Spacer()
                }
                                    
                Text("Lockbook")
                    .font(.largeTitle)
                    .fontWeight(.bold)
                    .padding(.leading)
                
                Text("The private note-taking platform.")
                    .font(.title2)
                    .padding(.leading)

                Text("The perfect place to record, sync, and share your thoughts.")
                    .font(.body)
                    .frame(maxWidth: 270)
                    .padding(.top)
                    .padding(.leading, 12)
                
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
                
                Text("By using Lockbook, you acknowledge our [Privacy Policy](https://lockbook.net/privacy-policy) and accept our [Terms of Service](https://lockbook.net/tos).")
                    .foregroundColor(.gray)
                    .font(.caption2)
                    .padding()
            }
            .padding(.top, 35)
            .padding(.horizontal)
        }
    }
}

#Preview("Onboarding 1") {
    OnboardingOneView()
}

struct OnboardingTwoView: View {
    @State var username: String = ""
    @State var createdAccount = false
    @State var keyPhrase: (String, String)? = nil
    
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
                .textInputAutocapitalization(.never)
                .onSubmit(createAccount)
                .padding(.top, 20)
            
            if let error = error {
                Text(error)
                    .foregroundStyle(.red)
                    .fontWeight(.bold)
                    .lineLimit(2, reservesSpace: false)
                    .padding(.top, 10)
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
            .padding(.top, 30)
            
            Spacer()
        }
        .padding(.top, 35)
        .padding(.horizontal, 25)
        .navigationDestination(isPresented: $createdAccount, destination: {
            if let keyPhrase = keyPhrase {
                OnboardingThreeView(username: username, keyPhrasePart1: keyPhrase.0, keyPhrasePart2: keyPhrase.1)
            }
        })
    }
    
    func createAccount() {
        working = true
        error = nil
        
        DispatchQueue.global(qos: .userInitiated).async {
            let operation = DI.core.createAccount(username: username, apiLocation: ConfigHelper.get(.apiLocation), welcomeDoc: true)
                switch operation {
                case .success:
                    switch DI.core.exportAccountPhrase() {
                    case .success(let phrase):
                        DispatchQueue.main.async {
                            let phrase = phrase.split(separator: " ")
                            let first12 = Array(phrase.prefix(12)).enumerated().map { (index, item) in
                                return "\(index + 1). \(item)"
                            }.joined(separator: "\n")
                            
                            let last12 = Array(phrase.suffix(12)).enumerated().map { (index, item) in
                                return "\(index + 13). \(item)"
                            }.joined(separator: "\n")
                            
                            keyPhrase = (first12, last12)
                            
                            createdAccount = true
                        }
                    case .failure(let err):
                        error = "Unexpected error."
                        DI.errors.handleError(err)
                    }
                    
                    break
                case .failure(let err):
                    DispatchQueue.main.async {
                        working = false
                        
                        switch err.kind {
                        case .UiError(let uiError):
                            switch uiError {
                            case .AccountExistsAlready:
                                error = "You already have an account, please file a bug report."
                            case .ClientUpdateRequired:
                                error = "Please download the most recent version."
                            case .CouldNotReachServer:
                                error = "Could not reach the server."
                            case .InvalidUsername:
                                error = "That username is invalid."
                            case .UsernameTaken:
                                error = "That username is not available."
                            case .ServerDisabled:
                                error = "The server is not accepting any new accounts at this moment. Please try again later."
                            }
                            break;
                        case .Unexpected:
                            error = "An unexpected error has occurred."
                            DI.errors.handleError(err)
                        }
                    }
                    break
                }
        }

    }
}

#Preview("Onboarding 2") {
    OnboardingTwoView()
}

struct OnboardingThreeView: View {
    let username: String
    let keyPhrasePart1: String
    let keyPhrasePart2: String
    
    @State var storedSecurely = false
    @State var working = false
    
    var body: some View {
        VStack(alignment: .leading) {
            Text("Your account key")
                .font(.title)
                .fontWeight(.bold)
            
            Text("This key confirms your identity and keeps your account secure. It's confidential and cannot be recovered if lost.")
                .padding(.top)
            
            Text("You can always access your key in the settings.")
                .padding(.top, 6)
                .padding(.bottom)
            
            HStack {
                VStack(alignment: .leading) {
                    ForEach(parseKeyPhrase(keyPhrasePart1), id: \.self) { phrase in
                        keyText(from: phrase)
                    }
                }
                .padding(.leading, 30)
                
                Spacer()
                
                VStack(alignment: .leading) {
                    ForEach(parseKeyPhrase(keyPhrasePart2), id: \.self) { phrase in
                        keyText(from: phrase)
                    }
                }
                .padding(.trailing, 30)
            }
            .frame(maxWidth: 350)
            .padding()
            .background(RoundedRectangle(cornerRadius: 6).foregroundStyle(.gray).opacity(0.1))

            Spacer()
            
            Toggle(isOn: $storedSecurely, label: {
                Text("I've stored my account key in safe place.")
                    .font(.callout)
                    .foregroundStyle(.primary)
            })
            .toggleStyle(iOSCheckboxToggleStyle())
            .padding(.top)
            .padding(.bottom)
            
            Button {
                DI.settings.copyAccountString()
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
        .padding(.horizontal, 25)
        .navigationBarBackButtonHidden()
    }
    
    func parseKeyPhrase(_ keyPhrase: String) -> [String] {
         return keyPhrase.components(separatedBy: "\n")
     }
     
     @ViewBuilder
     func keyText(from phrase: String) -> some View {
         let components = phrase.split(separator: " ", maxSplits: 1)
         
         if components.count == 2 {
             let number = components[0]
             let word = components[1]
             
             HStack {
                 Text("\(number)")
                     .foregroundColor(.blue)
                 
                 Text(word)
                     .foregroundColor(.primary)
                     .font(.system(.callout, design: .monospaced))
             }
         }
     }
    
    func goToMainScreen() {
        working = true
        DispatchQueue.global(qos: .userInitiated).async {
            DI.accounts.getAccount()
            DI.files.refresh()
        }
    }
}

#Preview("Onboarding 3") {
    OnboardingThreeView(username: "smail", keyPhrasePart1: "1. turkey\n2. era\n3. velvet\n4. detail\n5. prison\n6. income\n7. dose\n8. royal\n9. fever\n10. truly\n11. unique\n12. couple", keyPhrasePart2: "13. party\n14. example\n15. piece\n16. art\n17. leaf\n18. follow\n19. rose\n20. access\n21. vacant\n22. gather\n23. wasp\n24. audit")
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

struct ImportAccountView: View {
    @State var accountKey = ""
    @State var working = false
    @State var error: String? = nil
    
    @State var unsavedAPIURL: String = ""
    @State var apiURL: String = ""
    @State var importedAccount: Bool = false
    
    @State var showAPIURLSheet: Bool = false
    @State var showQRScanner: Bool = false
    
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
                    .modifier(DisableAutoCapitalization())
                    .padding(.trailing, 10)
                    .textFieldStyle(.roundedBorder)
                    .onSubmit {
                        importAccount(isAutoImporting: false)
                    }
                    .onChange(of: accountKey) { _ in
                        importAccount(isAutoImporting: true)
                    }

                Button(action: {
                    showQRScanner = true
                }, label: {
                    Image(systemName: "qrcode.viewfinder")
                        .font(.title)
                        .foregroundStyle(.blue)
                })
                .sheet(isPresented: $showQRScanner) {
                    CodeScannerView(codeTypes: [.qr], simulatedData: "This is simulated data", completion: handleScan)
                }
            }
            .padding(.top)
            
            if let error = error {
                Text(error)
                    .foregroundStyle(.red)
                    .fontWeight(.bold)
                    .lineLimit(2, reservesSpace: false)
                    .padding(.top)
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
                .padding(.bottom)
                .modifier(iOSAndiPadSheetViewModifier(isPresented: $showAPIURLSheet, width: 500, height: 130) {
                    SetAPIURLView(apiURL: $apiURL, unsavedAPIURL: apiURL)
                })
            }
        }
        .padding(.top, 35)
        .padding(.horizontal, 25)
        .navigationDestination(isPresented: $importedAccount, destination: {
            ImportAccountSyncView()
        })
        
    }
    
    func importAccount(isAutoImporting: Bool) {
        working = true
        let apiUrl: String? = if apiURL == "" {
            nil
        } else {
            apiURL
        }
        
        DispatchQueue.global(qos: .userInitiated).async {
            let res = DI.core.importAccount(accountString: accountKey, apiUrl: apiUrl)
            DispatchQueue.main.async {
                working = false
                
                switch res {
                case .success:
                    working = false
                    DI.sync.importSync()
                    importedAccount = true
                case .failure(let err):
                    switch err.kind {
                    case .UiError(let importError):
                        switch importError {
                        case .AccountDoesNotExist:
                            error = "That account does not exist on our server"
                        case .AccountExistsAlready:
                            error = "You already have an account, please file a bug report."
                        case .AccountStringCorrupted:
                            if !isAutoImporting {
                                error = "This account key is invalid."
                            }
                        case .ClientUpdateRequired:
                            error = "Please download the most recent version."
                        case .CouldNotReachServer:
                            error = "Could not reach the server."
                        case .UsernamePKMismatch:
                            error = "The account key's conveyed username does not match the public key stored on the server."
                        }
                    case .Unexpected:
                        error = "An unexpected error has occurred."
                        DI.errors.handleError(err)
                    }
                }
            }
        }
    }
    
    func handleScan(result: Result<String, CodeScannerView.ScanError>) {
        showQRScanner = false
        switch result {
        case .success(let key):
            accountKey = key
        case .failure(let err):
            print(err) // TODO: Convert this to an ApplicationError
        }
    }
}

struct SetAPIURLView: View {
    @Binding var apiURL: String

    @State var unsavedAPIURL = ""
    @FocusState var focused: Bool
    let defaultAPIURL: String = ConfigHelper.get(.apiLocation)
    
    @Environment(\.dismiss) private var dismiss
    
    var body: some View {
        VStack(spacing: 10) {
            HStack {
                Text("API URL")
                    .bold()
                
                Spacer()
            }
            
            TextField("Default: \(defaultAPIURL)", text: $unsavedAPIURL)
                .textFieldStyle(.roundedBorder)
                .autocorrectionDisabled()
                .textInputAutocapitalization(.never)
                .focused($focused)
                .onAppear {
                    focused = true
                }
            
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
    @EnvironmentObject var sync: SyncService
    
    var body: some View {
        VStack(spacing: 20) {
            Spacer()
            
            ProgressView(value: sync.syncProgress)
                .frame(maxWidth: 700)
            
            if let syncMsg = sync.syncMsg {
                Text(syncMsg)
                    .foregroundColor(.secondary)
            }
            
            Spacer()
        }
        .padding(.top, 35)
        .padding(.horizontal, 25)
        .navigationBarBackButtonHidden()
    }
}

//#Preview("Import Account Syncing") {
//    ImportAccountSyncView()
//        .mockDI()
//}
