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
                    
                    Button("Recalculate", action: calculateDebugInfo)
                }
            }
#endif
            
            if let debugInfo {
                ScrollView {
                    Spacer()
                    Button("Recalculate", action: calculateDebugInfo)
                    Text(debugInfo)
                        .monospaced()
                        .padding()
                        .textSelection(.enabled)
                    
                    Spacer()
                }
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
    
    func calculateDebugInfo() {
        DispatchQueue.global(qos: .userInitiated).async {
            let debug = AppState.lb.debugInfo()
            DispatchQueue.main.async {
                self.debugInfo = debug
            }
        }
    }
}

#Preview {
    NavigationStack {
        DebugView()
            .frame(width: 500, height: 500)
    }
}

