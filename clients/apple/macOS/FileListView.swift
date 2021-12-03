import SwiftUI
import SwiftLockbookCore

struct FileListView: View {
        
    @EnvironmentObject var coreService: CoreService
    @EnvironmentObject var files: FileService
    @EnvironmentObject var settings: SettingsService
    
    @StateObject var outlineState = OutlineState()
    
    let currentFolder: DecryptedFileMetadata
    let account: Account
    
    init(currentFolder: DecryptedFileMetadata, account: Account) {
        self.account = account
        self.currentFolder = currentFolder
    }
    
    var body: some View {
        VStack {
            OutlineSection(state: outlineState, root: currentFolder)
            VStack (spacing: 3) {
                if settings.showUsageAlert {
                    UsageBanner().onTapGesture {
                        NSApp.sendAction(Selector(("showPreferencesWindow:")), to: nil, from: nil)
                    }
                }
                Divider()
                BottomBar()
            }
        }
        .onAppear { // Different from willEnterForeground because its called on startup
            settings.calculateServerUsageDuringInitialLoad()
        }
        if let item = outlineState.selectedItem {
            DocumentView(meta: item)
        }
    }
}
