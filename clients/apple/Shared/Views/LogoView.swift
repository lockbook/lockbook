import SwiftUI

struct LogoView: View {
    #if os(iOS)
    static let image = Image(uiImage: UIImage(named: "logo")!)
    #else
    static let image = Image(nsImage: NSImage(named: NSImage.Name("logo"))!)
    #endif
                            
    var body: some View {
        
        HStack {
            LogoView
                .image
                .resizable()
                .scaledToFit()
                .frame(width: 75)
            VStack(alignment: .leading) {
                Text("Lockbook").font(.system(.largeTitle, design: .monospaced))
                Link("Learn more...", destination: URL(string: "https://lockbook.net")!)
                    .foregroundColor(.blue)
                    .padding(.leading, 3)
            }
        }
    }
}

struct LogoView_Previews: PreviewProvider {
    static var previews: some View {
        LogoView()
    }
}
