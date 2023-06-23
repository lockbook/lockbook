import SwiftUI
import SwiftLockbookCore
import Foundation

struct FileListView: View {
    @EnvironmentObject var current: CurrentDocument
    @EnvironmentObject var sheets: SheetState
    @EnvironmentObject var fileService: FileService
    @EnvironmentObject var search: SearchService
    @EnvironmentObject var sync: SyncService
    
    @State var searchInput: String = ""
    @State var navigateToManageSub: Bool = false
    @State private var hideOutOfSpaceAlert = UserDefaults.standard.bool(forKey: "hideOutOfSpaceAlert")
    
    var body: some View {
        VStack {
            if let newDoc = sheets.created, newDoc.fileType == .Document {
                NavigationLink(destination: DocumentView(meta: newDoc), isActive: Binding(get: { current.selectedDocument != nil }, set: { _ in current.selectedDocument = nil }) ) {
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
                    
                BottomBar(onCreating: {
                    if let parent = fileService.parent {
                        sheets.creatingInfo = CreatingInfo(parent: parent, child_type: .Document)
                    }
                })
                .onReceive(current.$selectedDocument) { _ in
                    print("cleared")
                    // When we return back to this screen, we have to change newFile back to nil regardless
                    // of it's present value, otherwise we won't be able to navigate to new, new files
                    if current.selectedDocument == nil {
                        sheets.created = nil
                    }
                }
        }
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
    
    @State private var offset = CGSize.zero
    @State private var opacity: Double = 1
    
    var mainView: some View {
        let children = fileService.childrenOfParent()
        
        return List {
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
                    ForEach(children) { meta in
                        VStack {
                            FileCell(meta: meta) {
                                withAnimation(.easeOut(duration: 0.2)) {
                                    offset.width = -500
                                }
                                
                                DispatchQueue.main.asyncAfter(deadline: .now() + 0.2) {
                                    opacity = 0
                                    offset.width = 500
                                    
                                    fileService.intoChildDirectory(meta)
                                    
                                    DispatchQueue.main.asyncAfter(deadline: .now() + 0.01) {
                                        withAnimation(.easeOut(duration: 0.1)) {
                                            offset.width = 0
                                            opacity = 1
                                        }
                                    }
                                }
                            }
                        }
                        .padding(.horizontal)
                        .padding(.vertical, 5)
                        .background(.white)
                    }
                    .listRowBackground(Color.clear)
                    .listRowInsets(EdgeInsets())
                    .listRowSeparator(.hidden)
            }
                .offset(offset)
                .opacity(opacity)

        }
        .navigationBarTitle(fileService.parent.map{($0.name)} ?? "")
        .gesture(DragGesture()
            .onChanged { gesture in
                if fileService.parent?.isRoot == false && gesture.translation.width < 150 && gesture.translation.width > 0 {
                    offset.width = gesture.translation.width
                }
            }
            .onEnded { gesture in
                if gesture.translation.width > 100 && fileService.parent?.isRoot == false {
                    withAnimation(.easeOut(duration: 0.2)) {
                        offset.width = 500
                        opacity = 0
                    }
                    
                    DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) {
                        offset.width = -500
                        opacity = 1
                                                
                        fileService.upADirectory()
                        
                        DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) {
                            withAnimation(.easeOut(duration: 0.1)) {
                                offset.width = 0
                            }
                        }
                    }
                } else {
                    withAnimation {
                        offset.width = 0
                    }
                }
            }
        )
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
