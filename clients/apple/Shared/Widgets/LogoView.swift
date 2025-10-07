import SwiftUI

struct LogoView: View {
    let scaledWidth: CGFloat = 75

    var body: some View {
        #if os(iOS)
            Image(uiImage: UIImage(named: "logo")!)
                .resizable()
                .scaledToFit()
                .frame(width: scaledWidth)
        #else
            Image(nsImage: NSImage(named: NSImage.Name("logo"))!)
                .resizable()
                .scaledToFit()
                .frame(width: scaledWidth)
        #endif
    }
}

#Preview {
    LogoView()
}
