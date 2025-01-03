import Foundation
import SwiftUI
import SwiftWorkspace
import AlertToast

struct PlatformView: View {
    @Environment(\.horizontalSizeClass) var horizontalSizeClass
    @ObservedObject var filesModel = FilesViewModel()
    
    var body: some View {
        if let error = filesModel.error {
            Text(error)
                .foregroundStyle(.red)
        } else if filesModel.loaded {
            NavigationSplitView {
                EmptyView()
            } detail: {
                EmptyView()
            }
            .navigationSplitViewStyle(.balanced)
        } else {
            ProgressView()
        }
    }
}

#Preview("Platform View") {
    PlatformView(filesModel: FilesViewModel(loaded: true))
}

#Preview("Platform View Loading") {
    PlatformView(filesModel: FilesViewModel(loaded: false))
}

#Preview("Platform View Error") {
    let model = FilesViewModel()
    model.error = "Failed to get files..."
    return PlatformView(filesModel: model)
}

