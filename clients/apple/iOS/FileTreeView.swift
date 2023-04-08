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
            VStack {
                switch search.searchPathAndContentState {
                case .NotSearching:
                    OutlineSection(root: currentFolder)
                case .NoMatch:
                    Text("NO match")
                case .Searching:
                    ProgressView()
                case .SearchSuccessful(let results):
                    List(results) { result in
                        switch result {
                        case .PathMatch(_, let name, let path, _, let matchedIndices):
                            SearchFilePathCell(name: name, path: path, matchedIndices: matchedIndices)
                        case .ContentMatch(_, let name, let path, let contentMatch):
                            SearchFileContentCell(name: name, path: path, paragraph: contentMatch.paragraph, matchedIndices: contentMatch.matchedIndices)
                        }
                    }
                }
            }
                .searchable(text: $searchInput)
                .onChange(of: searchInput) { [searchInput] newInput in
                    if(newInput.isEmpty && !searchInput.isEmpty) {
                        search.endSearch()
                    } else if (!newInput.isEmpty && searchInput.isEmpty) {
                        if(searchInput.isEmpty) {
                            search.startSearchThread()
                            return
                        }
                        
                        search.search(query: newInput)
                    }
                }

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
                }
            }
        }
    }
}
