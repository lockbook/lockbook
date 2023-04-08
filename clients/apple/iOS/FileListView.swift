import SwiftUI
import SwiftLockbookCore
import Foundation

struct FileListView: View {
    
    @Environment(\.isSearching) private var isSearching
    
    @EnvironmentObject var current: CurrentDocument
    @EnvironmentObject var sheets: SheetState
    @EnvironmentObject var fileService: FileService
    @EnvironmentObject var search: SearchService
    
    @State var searchInput: String = ""
    
    
    var body: some View {
            ZStack {
                VStack {
                    if let newDoc = sheets.created, newDoc.fileType == .Document {
                        NavigationLink(destination: DocumentView(meta: newDoc), isActive: Binding(get: { current.selectedDocument != nil }, set: { _ in current.selectedDocument = nil }) ) {
                             EmptyView()
                         }
                         .hidden()
                    }
                    
                    VStack {
                        switch search.searchPathAndContentState {
                        case .NotSearching:
                            List(fileService.childrenOfParent()) { meta in
                                FileCell(meta: meta)
                            }
                        case .NoMatch:
                            Text("No match")
                        case .Searching:
                            ProgressView()
                        case .SearchSuccessful(let results):
                            List(results) { result in
                                switch result {
                                case .PathMatch(let meta, let name, let path, _, let matchedIndices):
                                    NavigationLink(destination: DocumentView(meta: meta)) {
                                        SearchFilePathCell(name: name, path: path, matchedIndices: matchedIndices)
                                    }
                                case .ContentMatch(let meta, let name, let path, let contentMatch):
                                    NavigationLink(destination: DocumentView(meta: meta)) {
                                        SearchFileContentCell(name: name, path: path, paragraph: contentMatch.paragraph, matchedIndices: contentMatch.matchedIndices)
                                    }
                                }
                            }
                        }
                    }
                    .navigationBarTitle(fileService.parent.map{($0.name)} ?? "")
                    .searchable(text: $searchInput)
                    .onChange(of: searchInput) { [searchInput] newInput in
                        print("INPUTS: \(searchInput) \(newInput) and their conditions: \(newInput.isEmpty && !searchInput.isEmpty) and \(!newInput.isEmpty)")
                        if(newInput.isEmpty && !searchInput.isEmpty) {
                            search.endSearch()
                        } else if (!newInput.isEmpty) {
                            if(searchInput.isEmpty) {
                                search.startSearchThread()
                                return
                            }
                            
                            search.search(query: newInput)
                        }
                    }
                    
                    FilePathBreadcrumb()
                    
                    HStack {
                        BottomBar(onCreating: {
                            if let parent = fileService.parent {
                                sheets.creatingInfo = CreatingInfo(parent: parent, child_type: .Document)
                            }
                        })
                    }
                    .padding(.horizontal, 10)
                    .onReceive(current.$selectedDocument) { _ in
                        print("cleared")
                        // When we return back to this screen, we have to change newFile back to nil regardless
                        // of it's present value, otherwise we won't be able to navigate to new, new files
                        if current.selectedDocument == nil {
                            sheets.created = nil
                        }
                    }
                }
            }
            .gesture(
                DragGesture().onEnded({ (value) in
                    if value.translation.width > 50 && fileService.parent?.isRoot == false {
                        fileService.upADirectory()
                    }
                }))
    }
}

struct FileListView_Previews: PreviewProvider {
    static var previews: some View {
        NavigationView {
            FileListView()
                .mockDI()
        }
    }
}
