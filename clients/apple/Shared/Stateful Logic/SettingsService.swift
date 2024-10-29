import SwiftWorkspace
import SwiftUI

enum Tier {
    case Unknown
    case Trial
    case Premium
}

let PREMIUM_TIER_USAGE_CAP = 30000000000
let FREE_TIER_USAGE_CAP = 1000000

class SettingsService: ObservableObject {
    
    let core: Lb
    
    @Published var offline: Bool = false
    @Published var usages: PrerequisiteInformation?
    @Published var showView: Bool = false

    var usageProgress: Double {
        switch usages {
        case .some(let usage):
            return min(1.0, Double(usage.serverUsages.serverUsedExact) / Double(usage.serverUsages.serverCapExact))
        case .none:
            return 0
        }
    }
    
    var premiumProgress: Double {
        switch usages {
        case .some(let usage):
            return min(1.0, Double(usage.serverUsages.serverUsedExact) / Double(PREMIUM_TIER_USAGE_CAP))
        case .none:
            return 0
        }
    }
    
    var tier: Tier {
        switch usages {
        case .none:
            return .Unknown
        case .some(let wrapped):
            if wrapped.serverUsages.serverCapExact == FREE_TIER_USAGE_CAP {
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
    
    init(_ core: Lb) {
        self.core = core
    }
    
    func copyAccountString() {
        switch core.exportAccountPrivateKey() {
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
    
    func calculateUsage(calcUncompressed: Bool = false) {
        DispatchQueue.global(qos: .userInteractive).async {
            self.offline = false
            
            switch self.core.getUsage() {
            case .success(let usages):
                if calcUncompressed {
                    switch self.core.getUncompressedUsage() {
                    case .success(let uncompressedUsage):
                        DispatchQueue.main.async {
                            self.usages = PrerequisiteInformation(serverUsages: usages, uncompressedUsage: uncompressedUsage)
                        }
                    case .failure(let err):
                        switch err.code {
                        case .clientUpdateRequired:
                            DI.errors.errorWithTitle("Update Required", "You need to update to view your usage")
                            self.offline = false
                        case .serverUnreachable:
                            self.offline = true
                        default:
                            DI.errors.handleError(err)
                        }
                    }
                } else {
                    DispatchQueue.main.async {
                        self.usages = PrerequisiteInformation(serverUsages: usages, uncompressedUsage: nil)
                    }
                }
            case .failure(let err):
                switch err.code {
                case .serverUnreachable:
                    self.offline = true
                default:
                    DI.errors.handleError(err)
                }
            }
        }
    }
    
    func accountCode() -> AnyView {
        switch core.exportAccountPrivateKey() {
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
    let uncompressedUsage: UncompressedUsageMetric?
    var compressionRatio: String {
        if let uncompressedUsage = uncompressedUsage {
            let ratio = Double(uncompressedUsage.exact) / Double(serverUsages.serverUsedExact)
            return "\( round(ratio * 10) / 10.0 )x"
        } else {
            return "..."
        }
    }
}
