import SwiftUI
import SwiftWorkspace

struct DebugView: View {
    
    @State var debugInfo: String? = nil
    @State var copied = false
    
    var body: some View {
        VStack {
            #if os(macOS)
            if let debugInfo {
                HStack {
                    Text("Debug Info")
                    
                    Spacer()
                    
                    Button("Copy To Clipboard", action: {
                        ClipboardHelper.copyToClipboard(debugInfo)
                    })
                }
            }
            #endif
            
            if let debugInfo {
                ScrollView {
                    Spacer()
                    
                    Text(debugInfo)
                        .monospaced()
                        .padding()
                        .textSelection(.enabled)
                
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
            .frame(width: 500, height: 500)
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
