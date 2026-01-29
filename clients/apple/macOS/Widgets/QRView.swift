import SwiftUI
import CoreImage
import CoreImage.CIFilterBuiltins

struct QRView: View {
    let text: String
    
    var body: some View {
        if let qrImage = generateQRCode(text: text) {
            Image(nsImage: qrImage)
                .interpolation(.none)
                .resizable()
                .scaledToFit()
                .frame(width: 200, height: 200)
                .padding()
        } else {
            Text("Failed to generate QR Code")
        }
    }
    
    func generateQRCode(text: String) -> NSImage? {
        let context = CIContext()
        let filter = CIFilter.qrCodeGenerator()
        
        filter.message = Data(text.utf8)
        
        if let outputImage = filter.outputImage {
            let transform = CGAffineTransform(scaleX: 10, y: 10)
            let scaledImage = outputImage.transformed(by: transform)
            
            if let cgImage = context.createCGImage(scaledImage, from: scaledImage.extent) {
                let nsImage = NSImage(cgImage: cgImage, size: NSSize(width: scaledImage.extent.width, height: scaledImage.extent.height))
                return nsImage
            }
        }
        return nil
    }
}

#Preview {
    QRView(text: "turkey, era, velvet, detail, prison, income, dose, royal, fever, truly, unique, couple, party, example, piece, art, leaf, follow, rose, access, vacant, gather, wasp, audit")
}
