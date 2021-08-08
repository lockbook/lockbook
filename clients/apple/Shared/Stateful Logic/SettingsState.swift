import SwiftLockbookCore
import SwiftUI

struct PrerequisiteInformation {
    let serverUsages: UsageMetrics
    let uncompressedUsage: UsageItemMetric
    var compressionRatio: String {
        let ratio = Double(uncompressedUsage.exact) / Double(serverUsages.serverUsage.exact)
        return "\( round(ratio*10) / 10.0 )x"
    }
}

class SettingsState: ObservableObject {
    @ObservedObject var core: GlobalState
    
    @Published var usages: PrerequisiteInformation?
    var usageProgress: Double {
        switch usages {
        case .some(let usage):
            return min(1.0, Double(usage.serverUsages.serverUsage.exact) / Double(usage.serverUsages.dataCap.exact))
        case .none:
            return 0
        }
    }
    
    init(core: GlobalState) {
        self.core = core
    }
    
    // This is the actual right way to do async stuff
    func calculateUsage() {
        if self.usages == nil {
            DispatchQueue.global(qos: .userInteractive).async {
                switch self.core.api.getUsage() {
                case .success(let usages):
                    switch self.core.api.getUncompressedUsage() {
                    case .success(let uncompressedUsage):
                        DispatchQueue.main.async {
                            self.usages = PrerequisiteInformation(serverUsages: usages, uncompressedUsage: uncompressedUsage)
                        }
                    case .failure(let err):
                        self.core.handleError(err)
                    }
                case .failure(let err):
                    self.core.handleError(err)
                }
            }
        }
    }
    
    func accountCode() -> AnyView {
        switch core.api.exportAccount() {
        case .success(let accountString):
            let data = accountString.data(using: String.Encoding.ascii)
            if let filter = CIFilter(name: "CIQRCodeGenerator") {
                filter.setValue(data, forKey: "inputMessage")
                let transform = CGAffineTransform(scaleX: 3, y: 3)
                if let output = filter.outputImage?.transformed(by: transform) {
                    if let cgCode = CIContext().createCGImage(output, from: output.extent) {
                        return AnyView(Image(cgCode, scale: 1.0, label: Text("")))
                    }
                }
            }
        case .failure(let err):
            core.handleError(err)
        }
        return AnyView(Text("Failed to generate QR Code"))
    }
}
