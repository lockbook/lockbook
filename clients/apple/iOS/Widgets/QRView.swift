import SwiftUI
import CoreImage.CIFilterBuiltins

struct QRView: View {
    let text: String
    
    var body: some View {
        if let qrImage = generateQRCode(text: text) {
            Image(uiImage: qrImage)
                .interpolation(.none)
                .resizable()
                .scaledToFit()
                .frame(width: 200, height: 200)
                .padding()
        } else {
            Text("Failed to generate QR Code")
        }
    }
    
    func generateQRCode(text: String) -> UIImage? {
        let context = CIContext()
        let filter = CIFilter.qrCodeGenerator()
        
        filter.message = Data(text.utf8)
        
        if let outputImage = filter.outputImage {
            if let cgImage = context.createCGImage(outputImage, from: outputImage.extent) {
                return UIImage(cgImage: cgImage)
            }
        }
        return nil
    }
}

#Preview {
    QRView(text: "turkey, era, velvet, detail, prison, income, dose, royal, fever, truly, unique, couple, party, example, piece, art, leaf, follow, rose, access, vacant, gather, wasp, audit")
}


