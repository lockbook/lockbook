import SwiftUI

struct BeforeYouStart: View {
    
    @State var show = false
    
    var body: some View {
        VStack (spacing: 30){
            LogoView()
            Text("Lockbook [encrypts](https://en.wikipedia.org/wiki/End-to-end_encryption) your notes with a key that stays on your Lockbook devices. This makes your notes unreadable to anyone but you. If you lose the key, your notes are not recoverable, so we recommend you make a backup in case something happens to this device.")
                .font(.title2)
                .onAppear {
                    DispatchQueue.main.asyncAfter(deadline: .now() + .seconds(10)) {
                        withAnimation {
                            show = true
                        }
                    }
                }
            if show {
                VStack(spacing: 15) {
                    Button("Backup to another device") {
                        print("hi")
                    }
                    Button("I'll do this later") {
                        print("sad")
                    }.foregroundColor(.red)
                    
                }
            }
        }
    }
}

struct BeforeYouStartError: PreviewProvider {
    
    static var previews: some View {
        VStack {
            BeforeYouStart()
                .padding()
        }
    }
}
