import Foundation
import SwiftUI

struct OnboardingOneView: View {
    var body: some View {
        NavigationStack {
            VStack(alignment: .leading) {
                HStack {
                    logo
                    
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
    
    var logo: some View {
        #if os(iOS)
        Image(uiImage: UIImage(named: "logo")!)
            .resizable()
            .scaledToFit()
            .frame(width: 75)
        #else
        Image(nsImage: NSImage(named: NSImage.Name("logo"))!)
            .resizable()
            .scaledToFit()
            .frame(width: 75)
        #endif
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
                .modifier(DisableAutoCapitalization())
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
            if let keyPhrase = keyPhrase {
                OnboardingThreeView(username: username, keyPhrasePart1: keyPhrase.0, keyPhrasePart2: keyPhrase.1)
            }
        })
    }
    
    func createAccount() {
        working = true
        error = nil
        
        DispatchQueue.global(qos: .userInitiated).async {
            let operation = DI.core.createAccount(username: username, apiUrl: ConfigHelper.get(.apiLocation), welcomeDoc: true)
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
                    case .failure(_):
                        error = "An unexpected error has occurred."
                    }
                    
                    break
                case .failure(let err):
                    DispatchQueue.main.async {
                        working = false
                        error = err.msg
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
        .padding(.bottom)
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
            .modifier(iOSAndiPadSheetViewModifier(isPresented: $showAPIURLSheet, width: 500, height: 160) {
                SetAPIURLView(apiURL: $apiURL, unsavedAPIURL: apiURL)
            })
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
                .foregroundStyle(.blue)
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
            let res = DI.core.importAccount(key: accountKey, apiUrl: apiUrl)
            DispatchQueue.main.async {
                working = false
                
                switch res {
                case .success:
                    working = false
                    DI.sync.importSync()
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
        case .failure(let err):
            error = "Could not scan account key QR."
        }
    }
    #endif
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
            
            TextField("\(defaultAPIURL)", text: $unsavedAPIURL)
                .textFieldStyle(.roundedBorder)
                .autocorrectionDisabled()
                .modifier(DisableAutoCapitalization())
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
        .padding(.bottom)
        .padding(.horizontal, 25)
        .navigationBarBackButtonHidden()
    }
}
