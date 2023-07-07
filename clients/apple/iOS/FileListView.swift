import SwiftUI
import SwiftLockbookCore
import Foundation

struct FileListView: View {
    @EnvironmentObject var current: DocumentService
    @EnvironmentObject var sheets: SheetState
    @EnvironmentObject var fileService: FileService
    @EnvironmentObject var search: SearchService
    @EnvironmentObject var sync: SyncService
    
    @State var searchInput: String = ""
    @State var navigateToManageSub: Bool = false
    @State private var hideOutOfSpaceAlert = UserDefaults.standard.bool(forKey: "hideOutOfSpaceAlert")
    
    var body: some View {
        VStack {
            if let newDoc = current.justCreatedDoc, newDoc.fileType == .Document, current.openDocuments[newDoc.id] != nil {
                NavigationLink(destination: DocumentView(model: current.openDoc(meta: newDoc)), isActive: Binding(get: { current.openDocuments[newDoc.id] != nil }, set: { _ in current.openDocuments[newDoc.id] = nil }) ) {
                        EmptyView()
                    }
                    .hidden()
                }
                    
                SearchWrapperView(
                    searchInput: $searchInput,
                    mainView: mainView,
                    isiOS: true)
                .searchable(text: $searchInput, prompt: "Search")
                    
                FilePathBreadcrumb()
                    
                BottomBar(isiOS: true)
                .onReceive(current.$openDocuments) { _ in
                    print("cleared")
                    // When we return back to this screen, we have to change newFile back to nil regardless
                    // of it's present value, otherwise we won't be able to navigate to new, new files
                    if current.openDocuments.isEmpty {
                        current.justCreatedDoc = nil
                    }
                }
        }
        .gesture(
            DragGesture().onEnded({ (value) in
                if value.translation.width > 50 && fileService.parent?.isRoot == false {
                    fileService.upADirectory()
                }
            }))
        .alert(isPresented: Binding(get: { sync.outOfSpace && !hideOutOfSpaceAlert }, set: {_ in sync.outOfSpace = false })) {
            Alert(
                title: Text("Out of Space"),
                message: Text("You have run out of space!"),
                primaryButton: .default(Text("Upgrade now"), action: {
                    navigateToManageSub = true
                }),
                secondaryButton: .default(Text("Don't show me this again"), action: {
                    hideOutOfSpaceAlert = true
                    UserDefaults.standard.set(hideOutOfSpaceAlert, forKey: "hideOutOfSpaceAlert")
                })
            )
        }
        .background(
            NavigationLink(destination: ManageSubscription(), isActive: $navigateToManageSub, label: {
                EmptyView()
            })
            .hidden()
        )
    }
    
    var mainView: some View {
        List {
            if fileService.parent?.isRoot == true && fileService.suggestedDocs?.isEmpty != true {
                Section(header: Text("Suggested")
                    .bold()
                    .foregroundColor(.primary)
                    .textCase(.none)
                    .font(.headline)
                    .padding(.bottom, 3)) {
                        SuggestedDocs(isiOS: true)
                    }
            }

            Section(header: Text("Files")
                .bold()
                .foregroundColor(.primary)
                .textCase(.none)
                .font(.headline)
                .padding(.bottom, 3)) {
                ForEach(fileService.childrenOfParent()) { meta in
                    FileCell(meta: meta)
                }
            }
        }
        .navigationBarTitle(fileService.parent.map{($0.name)} ?? "")
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
