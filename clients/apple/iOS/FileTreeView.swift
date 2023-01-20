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
        } else {
            GeometryReader { geometry in
                if geometry.size.height > geometry.size.width {
                    VStack {
                        Image(systemName: "rectangle.portrait.lefthalf.inset.filled")
                            .font(.system(size: 60))
                            .padding(.bottom, 10)

                        
                        Text("No document is open. Expand the file tree by swiping from the left edge of the screen or clicking the button on the top left corner.")
                            .font(.title2)
                            .multilineTextAlignment(.center)
                            .frame(maxWidth: 350)
                    }
                    .padding(.horizontal)
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
                }
            }
        }
    }
}
