import SwiftUI
import SwiftWorkspace

struct DebugView: View {
    @State private var debugInfo: String? = nil
    @State private var chunks: [String] = []

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

            if debugInfo != nil {
                ScrollView {
                    LazyVStack(alignment: .leading, spacing: 0) {
                        ForEach(chunks.indices, id: \.self) { index in
                            Text(chunks[index])
                                .monospaced()
                        }
                    }
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
                if let debugInfo {
                    Button {
                        ClipboardHelper.copyToClipboard(debugInfo)
                    } label: {
                        Image(systemName: "doc.on.doc")
                    }
                }

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
            let debugChunks = Self.chunk(debug)

            DispatchQueue.main.async {
                debugInfo = debug
                chunks = debugChunks
            }
        }
    }

    static func chunk(_ text: String, limit: Int = 2000) -> [String] {
        var chunks: [String] = []
        var pending: [Substring] = []
        var pendingLength = 0

        for line in text.split(separator: "\n", omittingEmptySubsequences: false) {
            var rest = line
            repeat {
                let end = rest.index(rest.startIndex, offsetBy: limit, limitedBy: rest.endIndex) ?? rest.endIndex
                let segment = rest[..<end]
                rest = rest[end...]

                let length = segment.count
                if !pending.isEmpty && pendingLength + length > limit {
                    chunks.append(pending.joined(separator: "\n"))
                    pending = []
                    pendingLength = 0
                }
                pending.append(segment)
                pendingLength += length + 1
            } while !rest.isEmpty
        }

        chunks.append(pending.joined(separator: "\n"))
        return chunks
    }
}

#Preview {
    NavigationStack {
        DebugView()
    }
}
