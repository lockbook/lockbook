import SwiftUI
import SwiftLockbookCore

struct FileTreeView: View {

    @EnvironmentObject var sheets: SheetState
    @EnvironmentObject var currentDoc: CurrentDocument
    @EnvironmentObject var coreService: CoreService
    @EnvironmentObject var files: FileService
    @EnvironmentObject var onboarding: OnboardingService
    @EnvironmentObject var search: SearchService
    
    @State var searchInput: String = ""

    let currentFolder: File
    let account: Account
    
    var body: some View {
        VStack {
            SearchWrapperView(
                searchInput: $searchInput,
                mainView: mainView,
                isiOS: false)
            .searchable(text: $searchInput, placement: .navigationBarDrawer(displayMode: .automatic), prompt: "Search")

            HStack {
                BottomBar(onCreating: {
                    sheets.creatingInfo = CreatingInfo(parent: currentFolder, child_type: .Document)
                })
            }
        }
        
        VStack {
            if let item = currentDoc.selectedDocument {
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
                    } else {
                        EmptyView()
                    }
                }
            }
        }
        .toolbar {
            ToolbarItemGroup(placement: .navigationBarTrailing) {
                NavigationLink(
                    destination: PendingSharesView()) {
                        Image(systemName: "person.2.fill")
                            .foregroundColor(.blue)
                    }
                    
                NavigationLink(
                    destination: SettingsView().equatable(), isActive: $onboarding.theyChoseToBackup) {
                        Image(systemName: "gearshape.fill")
                            .foregroundColor(.blue)
                            .padding(.horizontal, 10)
                    }
            }
        }
        .onChange(of: currentDoc.selectedDocument) { _ in
            DI.files.refreshSuggestedDocs()
        }
    }
    
    var mainView: some View {
        List {
            if files.suggestedDocs?.isEmpty != true {
                Section(header: Text("Suggested")
                    .bold()
                    .foregroundColor(.primary)
                    .textCase(.none)
                    .font(.headline)
                    .padding(.bottom, 3)) {
                    SuggestedDocs(isiOS: false)
                }
                    .listRowSeparator(.hidden)
            }
            
            Section(header: Text("Files")
                .bold()
                .foregroundColor(.primary)
                .textCase(.none)
                .font(.headline)
                .padding(.bottom, 3)) {
                OutlineSection(root: currentFolder)
            }
            .listRowSeparator(.hidden)
        }
        .listStyle(.plain)
    }
}
