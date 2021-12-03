import SwiftLockbookCore
import SwiftUI

class SettingsService: ObservableObject {
    
    let core: LockbookApi
    
    // TODO give future consideration to what users will experience once they've paid and have run out of storage once again
    // how do these get reset?
    @AppStorage("usage_80_warning4") public var dismissed80 = false
    @AppStorage("usage_95_warning4") public var dismissed95 = false
    
    @Published var serverUsages: UsageMetrics?
    @Published var uncompressedUsage: UsageItemMetric?
    var compressionRatio: String {
        if let uncompressedUsage = uncompressedUsage, let serverUsages = serverUsages {
            let ratio = Double(uncompressedUsage.exact) / Double(serverUsages.serverUsage.exact)
            return "\( round(ratio*10) / 10.0 )x"
        } else {
            return "Calculating..."
        }
    }
    
    var usageProgress: Double {
        switch serverUsages {
        case .some(let usage):
            return min(1.0, Double(usage.serverUsage.exact) / Double(usage.dataCap.exact))
        case .none:
            return 0
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
    
    var showUsageAlert: Bool {
        if let usage = serverUsages {
            if Float64(usage.serverUsage.exact) / Float64(usage.dataCap.exact) >= 0.8 && !dismissed80 {
                return true
            }
            
            if Float64(usage.serverUsage.exact) / Float64(usage.dataCap.exact) >= 0.95 && !dismissed95 {
                return true
            }
        }
        return false
    }
    
    func dismissUsageDialog() {
        if usageProgress > 0.80 {
            dismissed80 = true
        }
        
        if usageProgress > 0.95 {
            dismissed80 = true
            dismissed95 = true
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
    
    /// There are 2 errors that could be experienced
    func calculateServerUsageDuringInitialLoad() {
        DispatchQueue.global(qos: .userInteractive).async {
            switch self.core.getUsage() {
            case .success(let usages):
                DispatchQueue.main.async {
                    self.serverUsages = usages
                }
            case .failure(let err):
                switch err.kind {
                case .UiError(let uiError):
                    switch uiError {
                    case .ClientUpdateRequired:
                        // Ignored because we are unable to handle this at startup and other user operations will trigger this and inform the user
                        break
                    case .CouldNotReachServer:
                        // Ignored because we are unable to handle this at startup and other user operations will trigger this and inform the user
                        break
                    default:
                        DI.errors.handleError(err)
                    }
                default:
                    DI.errors.handleError(err)
                }
            }
        }
    }
    
    func calculateUsage() {
        DispatchQueue.global(qos: .userInteractive).async {
            switch self.core.getUsage() {
            case .success(let usages):
                DispatchQueue.main.async {
                    self.serverUsages = usages
                }
                switch self.core.getUncompressedUsage() {
                case .success(let uncompressedUsage):
                    DispatchQueue.main.async {
                        self.uncompressedUsage = uncompressedUsage
                    }
                case .failure(let err):
                    // TODO handle an explicit offline mode here
                    switch err.kind {
                    case .UiError(let uiError):
                        switch uiError {
                        case .ClientUpdateRequired:
                            DI.errors.errorWithTitle("Update Required", "You need to update to view your usage")
                        case .CouldNotReachServer:
                            DI.errors.errorWithTitle("Offline", "Could not reach server to calculate usage")
                        default:
                            DI.errors.handleError(err)
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
