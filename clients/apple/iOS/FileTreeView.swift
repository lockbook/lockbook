import SwiftUI
import SwiftLockbookCore

struct FileTreeView: View {
        
    @EnvironmentObject var coreService: CoreService
    @EnvironmentObject var files: FileService
    @EnvironmentObject var onboarding: OnboardingService

    @StateObject var outlineState = OutlineState()
    
    let currentFolder: DecryptedFileMetadata
    let account: Account
    
    var body: some View {
        VStack {
            OutlineSection(state: outlineState, root: currentFolder)
            HStack {
                BottomBar()
            }
        }
        .toolbar {
            ToolbarItem(placement: .navigationBarTrailing) {
                NavigationLink(
                    destination: SettingsView().equatable(), isActive: $onboarding.theyChoseToBackup) {
                        Image(systemName: "gearshape.fill")
                            .foregroundColor(.blue)
                    }
            }
        }
        if let item = outlineState.selectedItem {
            DocumentView(meta: item)
        }
    }
}
