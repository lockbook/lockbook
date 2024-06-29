import SwiftUI
import Foundation

struct DebugView: View {
    
    @State var debugInfo: String = "Computing..."
    @State var copied = false
    
    var body: some View {
        VStack {
            ScrollView {
                Text(debugInfo)
                    .monospaced()
                    .onAppear {
                        DispatchQueue.global(qos: .background).async {
                            let debug = DI.core.debugInfo()
                            DispatchQueue.main.async {
                                debugInfo = debug
                            }
                        }
                    }
            }
            
            Button(action: copyDebug, label: {
                if copied {
                    Text("Copied")
                } else {
                    Text("Copy to clipboard")
                }
                
            })
        }
    }
    
    func copyDebug() {
        
        
#if os(iOS)
        UIPasteboard.general.string = debugInfo
#else
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(debugInfo, forType: .string)
#endif
        copied = true
        
        DispatchQueue.main.asyncAfter(deadline: .now() + .seconds(2)) {
            copied = false
        }
        
    }
    
}
