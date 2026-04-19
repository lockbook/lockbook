import SwiftUI
import SwiftWorkspace

struct SearchContainerSubView<Content: View>: View {
    @Binding var isSearching: Bool
    @ObservedObject var model: SearchContainerViewModel
    let dismissSearch: () -> Void

    let content: Content

    var body: some View {
        Group {
            if isSearching {
                VStack(spacing: 0) {
                    placeholder
                    SearchMetricsBar(model: model)
                }
            } else {
                content
            }
        }
        .onChange(of: isSearching) { newValue in
            if newValue {
                model.startSearching()
            } else {
                model.stopSearching()
            }
        }
    }

    var placeholder: some View {
        VStack(spacing: 12) {
            Spacer()
            Image(systemName: "magnifyingglass")
                .font(.largeTitle)
                .foregroundColor(.gray.opacity(0.6))
            Text("Search")
                .font(.headline)
                .foregroundColor(.gray)
            Text("UI coming soon.")
                .font(.caption)
                .foregroundColor(.gray.opacity(0.8))
            Spacer()
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

struct SearchMetricsBar: View {
    @ObservedObject var model: SearchContainerViewModel

    var body: some View {
        HStack(spacing: 12) {
            if let dur = model.buildDuration {
                metric(label: "build", value: format(ms: dur * 1000))
            }
            Spacer()
        }
        .font(.system(.caption2, design: .monospaced))
        .padding(.horizontal, 10)
        .padding(.vertical, 4)
        .background(Color.gray.opacity(0.08))
    }

    func metric(label: String, value: String) -> some View {
        HStack(spacing: 4) {
            Text(label).foregroundColor(.gray.opacity(0.7))
            Text(value).foregroundColor(.gray)
        }
    }

    func format(ms: Double) -> String {
        ms < 10 ? String(format: "%.2f ms", ms) : String(format: "%.0f ms", ms)
    }
}

class SearchContainerViewModel: ObservableObject {
    @Published var input: String = ""
    @Published var isShown: Bool = false
    @Published var buildDuration: TimeInterval? = nil

    let filesModel: FilesViewModel

    private var contentSearcher: ContentSearching?

    init(filesModel: FilesViewModel) {
        self.filesModel = filesModel
    }

    func startSearching() {
        guard contentSearcher == nil else { return }
        let start = Date()
        contentSearcher = AppState.lb.contentSearcher()
        buildDuration = Date().timeIntervalSince(start)
    }

    func stopSearching() {
        contentSearcher = nil
        buildDuration = nil
    }

    func search() {}
}
