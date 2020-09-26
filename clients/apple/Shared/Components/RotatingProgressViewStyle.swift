import SwiftUI

struct SyncIndicator: View {
    @Binding var syncing: Bool
    @State var spin: Bool = false
    var body: some View {
        if (syncing) {
            Image(systemName: "arrow.2.circlepath.circle.fill")
                .rotationEffect(.degrees(spin ? 360 : 0))
                .animation(Animation.linear(duration: 0.5).repeatForever(autoreverses: false))
                .onAppear() { spin.toggle() }
                .opacity(0.5)
        } else {
            Image(systemName: "arrow.2.circlepath.circle.fill")
        }
    }
}

struct SyncIndicator_Previews: PreviewProvider {
    static var previews: some View {
        SyncIndicator(syncing: .constant(true))
            .padding()
            .previewLayout(.sizeThatFits)
    }
}
