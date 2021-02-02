import SwiftUI
import SwiftLockbookCore

struct ImportAccountView: View {
    @ObservedObject var core: GlobalState
    @ObservedObject var onboardingState: OnboardingState
    
    @State var isScanning: Bool = false
    
    var body: some View {
        VStack(spacing: 40) {
            HStack {
                Text("Import an existing account")
                    .font(.title)
                    .bold()
                Spacer()
            }
            HStack {
                SecureField("Account String", text: self.$onboardingState.accountString, onCommit: self.onboardingState.handleImport)
                    .disableAutocorrection(true)
                    .autocapitalization(.none)
                    .textFieldStyle(RoundedBorderTextFieldStyle())
                    .disabled(self.onboardingState.working)
                Button(action: {
                    self.isScanning = true
                }) {
                    Image(systemName: "qrcode.viewfinder")
                }
                .frame(width: 40, height: 40)
                .disabled(self.onboardingState.working)
            }
            
            Text(onboardingState.importAccountError)
                .foregroundColor(.red)
                .bold()
        }
        .padding(.horizontal)
        .sheet(isPresented: self.$isScanning, content: {
            CodeScannerView(codeTypes: [.qr], simulatedData: "This is simulated data", completion: handleScan)
        })
        
    }
    
    func handleScan(result: Result<String, CodeScannerView.ScanError>) {
        self.isScanning = false
        switch result {
        case .success(let key):
            self.onboardingState.accountString = key
        case .failure(let err):
            print(err) // TODO: Convert this to an ApplicationError
        }
    }
}

struct ImportView_Previews: PreviewProvider {
    static var previews: some View {
        HStack {
            ImportAccountView(core: GlobalState(), onboardingState: OnboardingState(core: GlobalState()))
        }
    }
}

struct ImportViewWithError_Previews: PreviewProvider {
    static var onboardingState = OnboardingState(core: GlobalState())
    static var previews: some View {
        HStack {
            ImportAccountView(core: GlobalState(), onboardingState:onboardingState)
                .onAppear {
                    onboardingState.importAccountError = "Import error text!"
                }
        }
    }
}
