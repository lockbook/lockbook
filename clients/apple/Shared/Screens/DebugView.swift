import SwiftUI
import SwiftWorkspace

struct DebugView: View {
    
    @State var debugInfo: String? = nil
    @State var copied = false
    
    var body: some View {
        VStack {
            if let debugInfo {
                ScrollView {
                    Spacer()
                    
                    Text(debugInfo)
                        .monospaced()
                        .padding()
                
                    Spacer()
                }
                .modifier(CopyToClipboardViewmodifier(debugInfo: debugInfo))
            } else {
                Spacer()
                
                ProgressView()
                    .onAppear {
                        DispatchQueue.global(qos: .userInitiated).async {
                            let debug = AppState.lb.debugInfo()
                            DispatchQueue.main.async {
                                debugInfo = debug
                            }
                        }
                    }
                
                Spacer()
            }
        }
        .navigationTitle("Debug Info")
    }
}

#Preview {
    NavigationStack {
        DebugView()
    }
}

struct CopyToClipboardViewmodifier: ViewModifier {
    let debugInfo: String
    
    func body(content: Content) -> some View {
        #if os(iOS)
        content.toolbar {
            Button(action: {
                ClipboardHelper.copyToClipboard(debugInfo)
            }, label: {
                Image(systemName: "doc.on.doc")
            })
        }
        #else
        content
        #endif
        
    }
}
