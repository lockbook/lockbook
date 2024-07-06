import SwiftUI

struct BeforeYouStart: View {
    
    @Environment(\.presentationMode) var presentationMode
    
    var body: some View {
        VStack (spacing: 40){
            HStack {
                Text("Before you begin...")
                    .font(.title)
                    .bold()
                    .foregroundColor(.red)
                Spacer()
            }
            Text("Lockbook [encrypts](https://en.wikipedia.org/wiki/End-to-end_encryption) your notes with a key that stays on your Lockbook devices. This makes your notes unreadable to everyone except you. Therefore, if you lose this key, your notes are not recoverable. We recommend you make a backup in case something happens to this device.")
                .font(.title2)
                .fixedSize(horizontal: false, vertical: false)
            VStack(spacing: 20) {
                Button("Backup now!") {
                    #if os(iOS)
                    DI.settings.showView = true
                    #else
                    NSApp.sendAction(Selector(("showSettingsWindow:")), to: nil, from: nil)
                    #endif
                    presentationMode.wrappedValue.dismiss()
                }.foregroundColor(.blue)
                Button("I'll do this later") {
                    presentationMode.wrappedValue.dismiss()
                }.foregroundColor(.red)
            }
        }
        .padding()
        .frameForMacOS()
    }
}

extension View {
    func frameForMacOS() -> some View {
        #if os(macOS)
            return self.frame(width: 400, height: 400)
        #else
        return self
        #endif
    }
}

struct BeforeYouStartPreview: PreviewProvider {
    
    static var previews: some View {
        NavigationView {
            BeforeYouStart()
        }
    }
}
