import CoreImage.CIFilterBuiltins
import SwiftUI

struct QRView: View {
    let text: String

    var body: some View {
        if let qrImage = generateQRCode(text: text) {
            Image(decorative: qrImage, scale: 1)
                .interpolation(.none)
                .resizable()
                .scaledToFit()
                .frame(width: 200, height: 200)
                .padding()
        } else {
            Text("Failed to generate QR Code")
        }
    }

    func generateQRCode(text: String) -> CGImage? {
        let context = CIContext()
        let filter = CIFilter.qrCodeGenerator()

        filter.message = Data(text.utf8)

        guard let outputImage = filter.outputImage else {
            return nil
        }

        return context.createCGImage(outputImage, from: outputImage.extent)
    }
}

#Preview {
    QRView(text: "turkey, era, velvet, detail, prison, income, dose, royal, fever, truly, unique, couple, party, example, piece, art, leaf, follow, rose, access, vacant, gather, wasp, audit")
}
