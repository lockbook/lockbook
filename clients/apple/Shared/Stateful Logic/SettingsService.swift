import SwiftLockbookCore
import SwiftUI

enum Tier {
    case Unknown
    case Trial
    case Premium
}

class SettingsService: ObservableObject {
    
    let core: LockbookApi
    @Published var offline: Bool = false
    @Published var usages: PrerequisiteInformation?
    
    var usageProgress: Double {
        switch usages {
        case .some(let usage):
            return min(1.0, Double(usage.serverUsages.serverUsage.exact) / Double(usage.serverUsages.dataCap.exact))
        case .none:
            return 0
        }
    }
    
    var premiumProgress: Double {
        switch usages {
        case .some(let usage):
            return min(1.0, Double(usage.serverUsages.serverUsage.exact) / Double(30000000))
        case .none:
            return 0
        }
    }
    
    var tier: Tier {
        switch usages {
        case .none:
            return .Unknown
        case .some(let wrapped):
            if wrapped.serverUsages.dataCap.exact == 1000000 {
                return .Trial
            } else {
                return .Premium
            }
        }
    }
    
    @Published var copied: Bool = false {
        didSet {
            DispatchQueue.main.asyncAfter(deadline: .now() + 2) {
                self.copied = false
            }
        }
    }
    
    var copyToClipboardText: String {
        if copied {
            return "Copied"
        } else {
            return "Copy to clipboard"
        }
    }
    
    init(_ core: LockbookApi) {
        self.core = core
    }
    
    func copyAccountString() {
        switch core.exportAccount() {
        case .success(let accountString):
            #if os(iOS)
            UIPasteboard.general.string = accountString
            #else
            NSPasteboard.general.clearContents()
            NSPasteboard.general.setString(accountString, forType: .string)
            #endif
            copied = true
        case .failure(let err):
            DI.errors.handleError(err)
        }
    }
    
    func calculateUsage() {
        print("Recalculated usage...")
        
            DispatchQueue.global(qos: .userInteractive).async {
                switch self.core.getUsage() {
                case .success(let usages):
                    switch self.core.getUncompressedUsage() {
                    case .success(let uncompressedUsage):
                        print("Recalculated usage with \(uncompressedUsage.exact)")
                        DispatchQueue.main.async {
                            self.usages = PrerequisiteInformation(serverUsages: usages, uncompressedUsage: uncompressedUsage)
                            
                            if self.usageProgress < 0.8 && UserDefaults.standard.bool(forKey: "hideOutOfSpaceAlert") {
                                UserDefaults.standard.set(false, forKey: "hideOutOfSpaceAlert")
                            }
                        }
                    case .failure(let err):
                        // TODO handle an explicit offline mode here
                        switch err.kind {
                        case .UiError(let uiError):
                            switch uiError {
                            case .ClientUpdateRequired:
                                DI.errors.errorWithTitle("Update Required", "You need to update to view your usage")
                                self.offline = false
                            case .CouldNotReachServer:
                                self.offline = true
                            }
                        default:
                            DI.errors.handleError(err)
                        }
                        DI.errors.handleError(err)
                    }
                case .failure(let err):
                    DI.errors.handleError(err)
                }
            }
    }
    
    func accountCode() -> AnyView {
        switch core.exportAccount() {
        case .success(let accountString):
            let data = accountString.data(using: String.Encoding.ascii)
            if let filter = CIFilter(name: "CIQRCodeGenerator") {
                filter.setValue(data, forKey: "inputMessage")
                let transform = CGAffineTransform(scaleX: 3, y: 3)
                if let output = filter.outputImage?.transformed(by: transform) {
                    if let cgCode = CIContext().createCGImage(output, from: output.extent) {
                        return AnyView(Image(cgCode, scale: 1.0, label: Text(""))) // TODO make bigger probably 
                    }
                }
            }
        case .failure(let err):
            DI.errors.handleError(err)
        }
        return AnyView(Text("Failed to generate QR Code"))
    }
}

struct PrerequisiteInformation {
    let serverUsages: UsageMetrics
    let uncompressedUsage: UsageItemMetric
    var compressionRatio: String {
        let ratio = Double(uncompressedUsage.exact) / Double(serverUsages.serverUsage.exact)
        return "\( round(ratio*10) / 10.0 )x"
    }
}
