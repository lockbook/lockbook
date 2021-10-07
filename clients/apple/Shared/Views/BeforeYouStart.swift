import SwiftUI

struct BeforeYouStart: View {
    
    @Environment(\.presentationMode) var presentationMode
    
    // Let's do this by keeping a global variable that defaults to false that represents whether an account was created
    // this session. When an account is successfully created it's toggled to true. And we'll pop this bad boy up in a sheet
    // If they toggle on backup we'll pop open settings
    // If they say they'll do this later, we'll dismiss the sheet. 
    
    var body: some View {
        VStack (spacing: 40){
            HStack {
                Text("Before you begin...")
                    .font(.title)
                    .bold()
                    .foregroundColor(.red)
                Spacer()
            }
            Text("Lockbook [encrypts](https://en.wikipedia.org/wiki/End-to-end_encryption) your notes with a key that stays on your Lockbook devices. This makes your notes unreadable to anyone but you. If you lose the key, your notes are not recoverable, so we recommend you make a backup in case something happens to this device.")
                .font(.title2)
                .fixedSize(horizontal: false, vertical: false)
            VStack(spacing: 20) {
                Button("Backup now!") {
                    #if os(iOS)
                    DI.onboarding.theyChoseToBackup = true
                    #else
                    NSApp.sendAction(Selector(("showPreferencesWindow:")), to: nil, from: nil)
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
