import SwiftUI

struct LogoView: View {
    var body: some View {
        Image("logo")
            .resizable()
            .scaledToFit()
            .frame(width: 75)
    }
}

#Preview {
    LogoView()
}
