import SwiftUI
import SwiftWorkspace

struct DebugView: View {
    @State private var debugInfo: String? = nil

    var body: some View {
        VStack {
            #if os(macOS)
                if let debugInfo {
                    HStack {
                        Text("Debug Info")

                        Spacer()

                        Button("Copy To Clipboard") {
                            ClipboardHelper.copyToClipboard(debugInfo)
                        }

                        Button("Recalculate", action: calculateDebugInfo)
                    }
                }
            #endif

            if let debugInfo {
                ScrollView {
                    Text(debugInfo)
                        .monospaced()
                        .padding()
                        .textSelection(.enabled)
                }
            } else {
                Spacer()

                ProgressView()
                    .onAppear {
                        calculateDebugInfo()
                    }

                Spacer()
            }
        }
        #if os(iOS)
            .toolbar {
                Button(action: calculateDebugInfo) {
                    Image(systemName: "arrow.triangle.2.circlepath.circle")
                }
            }
        #endif
        .navigationTitle("Debug Info")
    }

    func calculateDebugInfo() {
        DispatchQueue.global(qos: .userInitiated).async {
            let debug = AppState.lb.debugInfo()

            DispatchQueue.main.async {
                debugInfo = debug
            }
        }
    }
}

#Preview {
    NavigationStack {
        DebugView()
    }
}
