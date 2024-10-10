import SwiftUI

struct QRScanner: View {
    
    @EnvironmentObject var onboardingState: OnboardingService
    
    @State var isScanning: Bool = false
    
    var body: some View {
        Button(action: {
            self.isScanning = true
        }) {
            Image(systemName: "qrcode.viewfinder")
        }
        .frame(width: 40, height: 40)
        .disabled(self.onboardingState.working)
        .sheet(isPresented: $isScanning) {
            CodeScannerView(codeTypes: [.qr], simulatedData: "This is simulated data", completion: handleScan)
        }
    }
    
    func handleScan(result: Result<String, CodeScannerView.ScanError>) {
        self.isScanning = false
        switch result {
        case .success(let key):
            self.onboardingState.accountString = key
            self.onboardingState.handleImport()
        case .failure(let err):
            print(err) // TODO: Convert this to an ApplicationError
        }
    }
}
