import SwiftUI
import SwiftLockbookCore

struct FileTreeView: View {

    @EnvironmentObject var sheets: SheetState
    @EnvironmentObject var currentDoc: CurrentDocument
    @EnvironmentObject var coreService: CoreService
    @EnvironmentObject var files: FileService
    @EnvironmentObject var onboarding: OnboardingService

    let currentFolder: File
    let account: Account
    
    var body: some View {
        VStack {
            OutlineSection(root: currentFolder)
            HStack {
                BottomBar(onCreating: {
                    sheets.creatingInfo = CreatingInfo(parent: currentFolder, child_type: .Document)
                })
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
        if let item = currentDoc.selectedItem {
            DocumentView(meta: item)
        }
    }
}
