import SwiftUI
import SwiftWorkspace

struct PlatformView: View {
    @Environment(\.horizontalSizeClass) var horizontalSizeClass
    @ObservedObject var filesModel = FilesViewModel()
    
    var body: some View {
        DrawerView(isOpened: false, menu: {
            
        }, content: {
            
        })
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
    PlatformView(filesModel: FilesViewModel(setLoaded: true))
}

#Preview("Platform View Loading") {
    PlatformView(filesModel: FilesViewModel(setLoaded: false))
}

#Preview("Platform View Error") {
    let model = FilesViewModel()
    model.error = "Failed to get files..."
    return PlatformView(filesModel: model)
}

